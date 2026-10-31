// SPDX-License-Identifier: MIT OR Apache-2.0
/*
 * TODO:
 *  - Field reads supporting custom handlers
 *  - Locked fields
 *  - Bank and index fields
 *  - Run `_REG` on supported op region handlers
 *  - Count operations performed and time
 *  - Correct DefStore / DefCopyObject behaviour
 *  - Load and LoadTable
 *  - DefDataRegion
 *  - Notify
 *  - DefMatch
 *
 *  - Method recursion depth?
 *  - Loop timeouts
 *  - Fuzzing and guarantee panic-free interpretation
 */

pub mod namespace;
pub mod object;
pub mod op_region;
pub mod pci_routing;
pub mod resource;

use crate::{
    AcpiError,
    AmlTable,
    Handle,
    Handler,
    PhysicalMapping,
    platform::AcpiPlatform,
    registers::{FixedRegisters, Pm1ControlBit},
    sdt::{SdtHeader, facs::Facs, fadt::Fadt},
};
use alloc::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
use bit_field::BitField;
use core::{
    mem,
    slice,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};
use log::{info, trace, warn};
use namespace::{AmlName, Namespace, NamespaceLevelKind};
use object::{
    DeviceStatus,
    FieldFlags,
    FieldUnit,
    FieldUnitKind,
    FieldUpdateRule,
    MethodFlags,
    Object,
    ObjectToken,
    ObjectType,
    ReferenceKind,
    WrappedObject,
};
use op_region::{OpRegion, RegionHandler, RegionSpace};
use pci_types::PciAddress;
use spinning_top::Spinlock;

/// `Interpreter` implements a virtual machine for the dynamic AML bytecode. It can be used by a
/// host operating system to load tables containing AML bytecode (generally the DSDT and SSDTs) and
/// will then manage the AML namespace and all objects created during the life of the system.
pub struct Interpreter<H>
where
    H: Handler,
{
    handler: H,
    pub namespace: Spinlock<Namespace>,
    pub object_token: Spinlock<ObjectToken>,
    context_stack: Spinlock<Vec<MethodContext>>,
    dsdt_revision: u8,
    region_handlers: Spinlock<BTreeMap<RegionSpace, Box<dyn RegionHandler>>>,

    global_lock_mutex: Handle,
    registers: Arc<FixedRegisters<H>>,
    facs: Option<PhysicalMapping<H, Facs>>,
}

unsafe impl<H> Send for Interpreter<H> where H: Handler + Send {}
unsafe impl<H> Sync for Interpreter<H> where H: Handler + Send {}

/// LOGANFY(Fix 1, c/d): fail closed on a feature the crate parses but does not implement. The
/// warning names the feature, since `AmlError::LibUnimplemented` carries no detail.
pub(crate) fn fail_closed(feature: &str) -> AmlError {
    warn!("LOGANFY fail-closed: {}", feature);
    AmlError::LibUnimplemented
}

/// LOGANFY(Fix 2): the largest object (Buffer bytes, Package elements, field bytes) AML may make the
/// interpreter allocate. Stock allocated whatever size the AML gave, so `Buffer (0xFFFFFFFF) {}`
/// ended the host on allocation failure. Real firmware objects are far smaller (the corpus's largest
/// Buffer is a few KiB); this is a policy limit, recorded in the Fix 2 findings.
pub const MAX_OBJECT_BYTES: usize = 1 << 20;

/// LOGANFY(Fix 2): an operator that builds a bigger object out of its operands (`Concat`,
/// `ConcatRes`, `ToHexString`, `ToDecimalString`, `ToBuffer` of a String) keeps its result within
/// `MAX_OBJECT_BYTES` too. The operands are within the limit already, so the transient allocation
/// before this check is bounded by a small multiple of it.
/// LOGANFY(Fix 3, 8): whether a Store to a zero-length Buffer gives it the source's length, as ACPICA
/// does (`exstorob.c` `AcpiExStoreBufferToBuffer`), instead of keeping it empty. J.c. decides; flip
/// this to keep the target's size in every case.
pub const STORE_GROWS_EMPTY_BUFFER: bool = true;

fn check_object_size(len: usize) -> Result<(), AmlError> {
    if len > MAX_OBJECT_BYTES {
        return Err(AmlError::MalformedAml("result larger than the interpreter's object limit"));
    }
    Ok(())
}

/// LOGANFY(Fix 2): how deeply blocks (scopes, packages, If/While bodies), method calls and in-flight
/// operators may nest. Each level is heap state here, but the objects they build are dropped and
/// printed recursively; unbounded method recursion grew `context_stack`, and a loop whose body left
/// an operator without its operands grew `in_flight`, until allocation failed. Policy limits,
/// recorded in the Fix 2 findings.
pub const MAX_NESTING: usize = 256;

/// LOGANFY(Fix 2): fail closed rather than store an object that would make the graph cyclic
/// (`Store (RefOf (Local0), Local0)`: four bytes of AML that made `typ()` recurse until the stack
/// overflowed).
fn check_acyclic(target: &WrappedObject, value: &WrappedObject) -> Result<(), AmlError> {
    let mut budget = 1 << 16;
    if value.reaches(target, MAX_NESTING, &mut budget) {
        Err(AmlError::MalformedAml("store that would make an object refer to itself, or nest too deeply"))
    } else {
        Ok(())
    }
}

/// LOGANFY(Fix 2): a buffer field is created over a Buffer, and must lie inside it (ACPI 6.5 §19.6
/// `CreateField`: SourceBuffer evaluates to a Buffer; ACPICA rejects a field past the end with
/// AE_AML_BUFFER_LIMIT). Stock accepted any object and any offset, then panicked or wrote out of
/// bounds on access, and allocated `length / 8` bytes on read whatever `length` was.
fn check_buffer_field(buffer: &WrappedObject, offset: u64, length: u64) -> Result<(usize, usize), AmlError> {
    let Object::Buffer(ref bytes) = **buffer else {
        return Err(AmlError::ObjectNotOfExpectedType { expected: ObjectType::Buffer, got: buffer.typ() });
    };
    match offset.checked_add(length) {
        Some(end) if length > 0 && end <= bytes.len() as u64 * 8 => Ok((offset as usize, length as usize)),
        _ => Err(AmlError::MalformedAml("buffer field outside its buffer")),
    }
}

/// The value returned by the `Revision` opcode.
const INTERPRETER_REVISION: u64 = 1;

impl<H> Interpreter<H>
where
    H: Handler,
{
    /// Construct a new `Interpreter`. This does not load any tables - if you have an `AcpiTables`
    /// already, use [`Interpreter::new_from_tables`] instead.
    pub fn new(
        handler: H,
        dsdt_revision: u8,
        registers: Arc<FixedRegisters<H>>,
        facs: Option<PhysicalMapping<H, Facs>>,
    ) -> Interpreter<H> {
        info!("Initializing AML interpreter v{}", env!("CARGO_PKG_VERSION"));

        let global_lock_mutex = handler.create_mutex();

        Interpreter {
            handler,
            namespace: Spinlock::new(Namespace::new(global_lock_mutex)),
            object_token: Spinlock::new(unsafe { ObjectToken::create_interpreter_token() }),
            context_stack: Spinlock::new(Vec::new()),
            dsdt_revision,
            region_handlers: Spinlock::new(BTreeMap::new()),
            global_lock_mutex,
            registers,
            facs,
        }
    }

    /// Construct a new `Interpreter` with the given set of ACPI tables. This will automatically
    /// load the DSDT and any SSDTs in the supplied [`AcpiTables`].
    pub fn new_from_platform(platform: &AcpiPlatform<H>) -> Result<Interpreter<H>, AcpiError> {
        fn load_table(interpreter: &Interpreter<impl Handler>, table: AmlTable) -> Result<(), AcpiError> {
            let mapping = unsafe {
                interpreter.handler.map_physical_region::<SdtHeader>(table.phys_address, table.length as usize)
            };
            let stream = unsafe {
                slice::from_raw_parts(
                    mapping.virtual_start.as_ptr().byte_add(mem::size_of::<SdtHeader>()) as *const u8,
                    table.length as usize - mem::size_of::<SdtHeader>(),
                )
            };
            interpreter.load_table(stream).map_err(AcpiError::Aml)?;
            Ok(())
        }

        let registers = platform.registers.clone();
        let facs = {
            platform.tables.find_table::<Fadt>().and_then(|fadt| fadt.facs_address().ok()).map(
                |facs_address| unsafe {
                    platform.handler.map_physical_region(facs_address, mem::size_of::<Facs>())
                },
            )
        };

        let dsdt = platform.tables.dsdt()?;
        let interpreter = Interpreter::new(platform.handler.clone(), dsdt.revision, registers, facs);
        load_table(&interpreter, dsdt)?;

        for ssdt in platform.tables.ssdts() {
            load_table(&interpreter, ssdt)?;
        }

        Ok(interpreter)
    }

    /// Load the supplied byte stream as an AML table. This should be only the encoded AML stream -
    /// not the header at the start of a table. If you've used [`Interpreter::new_from_tables`],
    /// you'll likely not need to load any tables manually.
    pub fn load_table(&self, stream: &[u8]) -> Result<(), AmlError> {
        let context = unsafe { MethodContext::new_from_table(stream) };
        self.do_execute_method(context)?;
        Ok(())
    }

    /// Evaluate an object at the given path in the namespace. If the object is a method, this
    /// invokes the method with the given set of arguments.
    pub fn evaluate(&self, path: AmlName, args: Vec<WrappedObject>) -> Result<WrappedObject, AmlError> {
        trace!("Invoking AML method: {}", path);

        let object = self.namespace.lock().get(path.clone())?.clone();
        match &*object {
            Object::Method { .. } => {
                self.namespace.lock().add_level(path.clone(), NamespaceLevelKind::MethodLocals)?;
                let context = MethodContext::new_from_method(object, args, path)?;
                self.do_execute_method(context)
            }
            Object::NativeMethod { f, .. } => f(&args),
            _ => Ok(object),
        }
    }

    pub fn evaluate_if_present(
        &self,
        path: AmlName,
        args: Vec<WrappedObject>,
    ) -> Result<Option<WrappedObject>, AmlError> {
        match self.evaluate(path.clone(), args) {
            Ok(result) => Ok(Some(result)),
            Err(AmlError::ObjectDoesNotExist(not_present)) => {
                if path == not_present {
                    Ok(None)
                } else {
                    Err(AmlError::ObjectDoesNotExist(not_present))
                }
            }
            Err(other) => Err(other),
        }
    }

    pub fn install_region_handler<RH>(&self, space: RegionSpace, handler: RH)
    where
        RH: RegionHandler + 'static,
    {
        let mut handlers = self.region_handlers.lock();
        assert!(handlers.get(&space).is_none(), "Tried to install handler for same space twice!");
        handlers.insert(space, Box::new(handler));
    }

    /// Initialize the namespace - this should be called after all tables have been loaded and
    /// operation region handlers registered. Specifically, it will call relevant `_STA`, `_INI`,
    /// and `_REG` methods.
    pub fn initialize_namespace(&self) {
        /*
         * This should match the initialization order of ACPICA and uACPI.
         */
        if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_INI").unwrap(), vec![]) {
            warn!("Invoking \\_INI failed: {:?}", err);
        }
        if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_SB._INI").unwrap(), vec![]) {
            warn!("Invoking \\_SB._INI failed: {:?}", err);
        }

        // TODO: run all _REGs for globally-installed handlers (this might need more bookkeeping)

        /*
         * We can now initialize each device in the namespace. For each device, we evaluate `_STA`,
         * which indicates if the device is present and functional. If this method does not exist,
         * we assume the device should be initialized.
         *
         * We then evaluate `_INI` for the device. This can dynamically populate objects such as
         * `_ADR`, `_CID`, `_HID`, `_SUN`, and `_UID`, and so is necessary before further
         * operation.
         */
        let mut num_devices_initialized = 0;
        /*
         * TODO
         * We clone a copy of the namespace here to traverse while executing all the `_STA` and
         * `_INI` objects. Avoiding this would be good, but is not easy, as we need
         * potentially-mutable access while executing all of the methods.
         */
        let mut namespace = self.namespace.lock().clone();
        let init_status = namespace.traverse(|path, level| {
            match level.kind {
                NamespaceLevelKind::Device
                | NamespaceLevelKind::Processor
                | NamespaceLevelKind::ThermalZone
                | NamespaceLevelKind::PowerResource => {
                    let should_initialize = match self
                        .evaluate_if_present(AmlName::from_str("_STA").unwrap().resolve(path)?, vec![])
                    {
                        Ok(Some(result)) => {
                            let Object::Integer(result) = *result else {
                                return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                            };
                            let status = DeviceStatus(result);
                            status.present() && status.functioning()
                        }
                        Ok(None) => true,
                        Err(err) => {
                            warn!("Failed to evaluate _STA for device {}: {:?}", path, err);
                            false
                        }
                    };

                    if should_initialize {
                        num_devices_initialized += 1;
                        if let Err(err) =
                            self.evaluate_if_present(AmlName::from_str("_INI").unwrap().resolve(path)?, vec![])
                        {
                            warn!("Failed to evaluate _INI for device {}: {:?}", path, err);
                        }
                        Ok(true)
                    } else {
                        /*
                         * If this device should not be initialized, don't initialize it's children.
                         */
                        Ok(false)
                    }
                }
                _ => Ok(true),
            }
        });
        if let Err(err) = init_status {
            warn!("Error while traversing namespace for devices: {:?}", err);
        }

        info!("Initialized {} devices", num_devices_initialized);
    }

    pub fn acquire_global_lock(&self, timeout: u16) -> Result<(), AmlError> {
        self.handler.acquire(self.global_lock_mutex, timeout)?;

        // Now we've acquired the AML-side mutex, acquire the hardware side
        // TODO: count the number of times we have to go round this loop / enforce a timeout?
        loop {
            if self.try_do_acquire_firmware_lock() {
                break Ok(());
            } else {
                /*
                 * The lock is owned by the firmware. We have set the pending bit - we now need to
                 * wait for the firmware to signal it has released the lock.
                 *
                 * TODO: this should wait for an interrupt from the firmware. That needs more infra
                 * so for now let's just spin round and try and acquire it again...
                 */
                self.handler.release(self.global_lock_mutex);
                continue;
            }
        }
    }

    /// Attempt to acquire the firmware lock, setting the owned bit if the lock is free. If the
    /// lock is not free, sets the pending bit to instruct the firmware to alert us when we can
    /// attempt to take ownership of the lock again. Returns `true` if we now have ownership of the
    /// lock, and `false` if we need to wait for firmware to release it.
    fn try_do_acquire_firmware_lock(&self) -> bool {
        let Some(facs) = &self.facs else { return true };
        loop {
            let global_lock = facs.global_lock.load(Ordering::Relaxed);
            let is_owned = global_lock.get_bit(1);

            /*
             * Compute the new value: either the lock is already owned, and we need to set the
             * pending bit and wait, or we can acquire ownership of the lock now. Either way, we
             * unconditionally set the owned bit and set the pending bit if the lock is already
             * owned.
             */
            let mut new_value = global_lock;
            new_value.set_bit(0, is_owned);
            new_value.set_bit(1, true);

            if facs
                .global_lock
                .compare_exchange(global_lock, new_value, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break !is_owned;
            }
        }
    }

    pub fn release_global_lock(&self) -> Result<(), AmlError> {
        let is_pending = self.do_release_firmware_lock();
        if is_pending {
            // LOGANFY(Fix 2): stock unwraps; `Release (\_GL)` reaches this, and the PM1 access can fail
            // on the host's register handler. Fail closed instead.
            self.registers
                .pm1_control_registers
                .set_bit(Pm1ControlBit::GlobalLockRelease, true)
                .map_err(|_| fail_closed("PM1 GlobalLockRelease write"))?;
        }
        Ok(())
    }

    /// Atomically release the owned and pending bits of the global lock. Returns whether the
    /// pending bit was set (this means the firmware is waiting to acquire the lock, and should be
    /// informed we're finished with it).
    fn do_release_firmware_lock(&self) -> bool {
        let Some(facs) = &self.facs else { return false };
        loop {
            let global_lock = facs.global_lock.load(Ordering::Relaxed);
            let is_pending = global_lock.get_bit(0);
            let mut new_value = global_lock;
            new_value.set_bit(0, false);
            new_value.set_bit(1, false);

            if facs
                .global_lock
                .compare_exchange(global_lock, new_value, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break is_pending;
            }
        }
    }

    fn do_execute_method(&self, mut context: MethodContext) -> Result<WrappedObject, AmlError> {
        /*
         * LOGANFY(Fix 1, b): a nested method call pushes its caller's context onto the shared
         * `context_stack` and adds a `MethodLocals` namespace level, and only a normal method
         * exit undid either. An error (or a panic) left both behind, so a later evaluation's
         * `Return` popped a stale caller context and ran on inside it. The same pop also let a
         * re-entrant evaluation (`_ADR`/`_SEG`/`_BBR`, run while a PCI_Config field access is in
         * progress) return into the outer evaluation's caller. Record the stack depth on entry:
         * this execution only pops contexts it pushed (`pop_caller_context`), and on an error
         * exit it drops every context it pushed and removes the method level of each method it
         * had entered, including the one `evaluate` entered for it.
         */
        let base = self.context_stack.lock().len();
        let result = self.do_execute_method_inner(&mut context, base);
        if result.is_err() {
            let mut scopes = Vec::new();
            scopes.extend(context.method_scope().cloned());
            {
                let mut stack = self.context_stack.lock();
                while stack.len() > base {
                    let caller = stack.pop().unwrap();
                    scopes.extend(caller.method_scope().cloned());
                }
            }
            let mut namespace = self.namespace.lock();
            for scope in scopes {
                let _ = namespace.remove_level(scope);
            }
        }
        result
    }

    fn do_execute_method_inner(
        &self,
        context: &mut MethodContext,
        base: usize,
    ) -> Result<WrappedObject, AmlError> {
        /*
         * This is the main loop that executes operations. Every op is handled at the top-level of
         * the loop to prevent pathological stack growth from nested operations.
         *
         * The loop has three main stages:
         *   1) Check if any in-flight operations are ready to be executed (i.e. have collected all
         *      their arguments). An operation completing may contribute the last required argument
         *      of the one above, so this is repeated for as many operations as are ready to be
         *      retired.
         *   2) Look at the next opcode in the stream. If we've run out of opcodes in the current
         *      block, run logic to determine where in the stream we should move to next. Special
         *      logic at this level handles things like moving in/out of package definitions, and
         *      performing control flow.
         *   3) When the next opcode is determined, use it to interpret the next portion of the
         *      stream. If that is data, the correct number of bytes can be consumed and
         *      contributed to the current in-flight operation. If it's an opcode, a new in-flight
         *      operation is started, and we go round the loop again.
         *
         * This scheme is what allows the interpreter to use a loop that somewhat resembles a
         * traditional fast bytecode VM, but also provides enough flexibility to handle the
         * quirkier parts of the AML grammar, particularly the left-to-right encoding of operands.
         */
        loop {
            // LOGANFY(Fix 2): bounded operator nesting (see MAX_NESTING).
            if context.in_flight.len() > MAX_NESTING {
                return Err(AmlError::MalformedAml("operators nested deeper than the interpreter's limit"));
            }

            /*
             * First, see if we've gathered enough arguments to complete some in-flight operations.
             */
            while let Some(op) = context.in_flight.pop_if(|op| op.arguments.len() == op.expected_arguments) {
                match op.op {
                    Opcode::Add
                    | Opcode::Subtract
                    | Opcode::Multiply
                    | Opcode::Divide
                    | Opcode::ShiftLeft
                    | Opcode::ShiftRight
                    | Opcode::Mod
                    | Opcode::Nand
                    | Opcode::And
                    | Opcode::Or
                    | Opcode::Nor
                    | Opcode::Xor => {
                        self.do_binary_maths(context, op)?;
                    }
                    Opcode::Not | Opcode::FindSetLeftBit | Opcode::FindSetRightBit => {
                        self.do_unary_maths(context, op)?;
                    }
                    Opcode::Increment | Opcode::Decrement => {
                        /*
                         * LOGANFY(Fix 1, G1): `Increment (Local0)` names a Local, which arrives as a
                         * transparent reference, and `Increment (FLD)` a field unit; stock required a
                         * bare Integer object and failed both. Read the operand's value, then store
                         * the result back through the same SuperName and return it (ACPI 6.5 §19.6
                         * `Increment`, `Decrement`).
                         */
                        let [Argument::Object(operand)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let current = operand.clone().unwrap_transparent_reference();
                        let current = match *current {
                            Object::Reference { kind: ReferenceKind::RefOf | ReferenceKind::Index, ref inner } => {
                                inner.clone()
                            }
                            _ => current.clone(),
                        };
                        let value = self.read_named_value(current)?.as_integer()?;

                        let new_value = match op.op {
                            Opcode::Increment => value.wrapping_add(1),
                            Opcode::Decrement => value.wrapping_sub(1),
                            _ => unreachable!(),
                        };

                        let result =
                            self.do_store(&Argument::Object(operand.clone()), Object::Integer(new_value).wrap())?;
                        context.contribute_arg(Argument::Object(result));
                        context.retire_op(op);
                    }
                    Opcode::LAnd
                    | Opcode::LOr
                    | Opcode::LNot
                    | Opcode::LNotEqual
                    | Opcode::LLessEqual
                    | Opcode::LGreaterEqual
                    | Opcode::LEqual
                    | Opcode::LGreater
                    | Opcode::LLess => {
                        self.do_logical_op(context, op)?;
                    }
                    Opcode::ToBuffer => self.do_to_buffer(context, op)?,
                    Opcode::ToInteger => self.do_to_integer(context, op)?,
                    Opcode::ToString => self.do_to_string(context, op)?,
                    Opcode::ToDecimalString | Opcode::ToHexString => self.do_to_dec_hex_string(context, op)?,
                    Opcode::Mid => self.do_mid(context, op)?,
                    Opcode::Concat => self.do_concat(context, op)?,
                    Opcode::ConcatRes => {
                        let [Argument::Object(source1), Argument::Object(source2), target] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        let source1 = source1.clone().unwrap_transparent_reference();
                        let source2 = source2.clone().unwrap_transparent_reference();
                        let source1 = source1.as_buffer()?;
                        let source2 = source2.as_buffer()?;
                        let result = {
                            let mut buffer = Vec::from(source1);
                            buffer.extend_from_slice(source2);
                            // Add a new end-tag
                            buffer.push(0x78);
                            // Don't calculate the new real checksum - just use 0
                            buffer.push(0x00);
                            check_object_size(buffer.len())?;
                            Object::Buffer(buffer).wrap()
                        };
                        // TODO: use potentially-updated result for return value here
                        self.do_store(target, result.clone())?;
                        context.contribute_arg(Argument::Object(result));
                        context.retire_op(op);
                    }
                    Opcode::Reset => {
                        let [Argument::Object(sync_object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let sync_object = sync_object.clone().unwrap_reference();

                        if let Object::Event(ref counter) = *sync_object {
                            counter.store(0, Ordering::Release);
                        } else {
                            return Err(AmlError::InvalidOperationOnObject {
                                op: Operation::ResetEvent,
                                typ: sync_object.typ(),
                            });
                        }
                    }
                    Opcode::Signal => {
                        let [Argument::Object(sync_object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let sync_object = sync_object.clone().unwrap_reference();

                        if let Object::Event(ref counter) = *sync_object {
                            counter.fetch_add(1, Ordering::AcqRel);
                        } else {
                            return Err(AmlError::InvalidOperationOnObject {
                                op: Operation::SignalEvent,
                                typ: sync_object.typ(),
                            });
                        }
                    }
                    Opcode::Wait => {
                        let [Argument::Object(sync_object), Argument::Object(timeout)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let sync_object = sync_object.clone().unwrap_reference();
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        let timeout =
                            u64::min(timeout.clone().unwrap_transparent_reference().as_integer()?, 0xffff);

                        if let Object::Event(ref counter) = *sync_object {
                            /*
                             * `Wait` returns a non-zero value if a timeout occurs and the event
                             * was not signaled, and zero if it was. Timeout is specified in
                             * milliseconds, should relinquish processor control (we use
                             * `Handler::sleep` to do so) and a value of `0xffff` specifies that
                             * the operation should wait indefinitely.
                             */
                            let mut remaining_sleep = timeout;
                            let mut timed_out = true;

                            'signaled: while remaining_sleep > 0 {
                                loop {
                                    /*
                                     * Try to decrement the counter. If it's zero after a load, we
                                     * haven't been signalled and should wait for a bit. If it's
                                     * non-zero, we were signalled and should stop waiting.
                                     */
                                    let value = counter.load(Ordering::Acquire);
                                    if value == 0 {
                                        break;
                                    }
                                    if counter
                                        .compare_exchange(value, value - 1, Ordering::AcqRel, Ordering::Acquire)
                                        .is_ok()
                                    {
                                        timed_out = false;
                                        break 'signaled;
                                    }
                                }

                                let to_sleep = u64::min(timeout, 10);
                                if timeout < 0xffff {
                                    remaining_sleep -= to_sleep
                                }
                                self.handler.sleep(to_sleep);
                            }

                            context.contribute_arg(Argument::Object(
                                Object::Integer(if timed_out { u64::MAX } else { 0 }).wrap(),
                            ));
                        } else {
                            return Err(AmlError::InvalidOperationOnObject {
                                op: Operation::WaitEvent,
                                typ: sync_object.typ(),
                            });
                        }
                    }
                    Opcode::FromBCD => self.do_from_bcd(context, op)?,
                    Opcode::ToBCD => self.do_to_bcd(context, op)?,
                    Opcode::Name => {
                        let [Argument::Namestring(name), Argument::Object(object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };

                        let name = name.resolve(&context.current_scope)?;
                        self.namespace.lock().insert(name, object.clone())?;
                        context.retire_op(op);
                    }
                    Opcode::Fatal => {
                        let [Argument::ByteData(typ), Argument::DWordData(code), Argument::Object(arg)] =
                            &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        let arg = arg.clone().unwrap_transparent_reference().as_integer()?;
                        self.handler.handle_fatal_error(*typ, *code, arg);
                        context.retire_op(op);
                    }
                    Opcode::OpRegion => {
                        let [
                            Argument::Namestring(name),
                            Argument::ByteData(region_space),
                            Argument::Object(region_offset),
                            Argument::Object(region_length),
                        ] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };

                        let region_offset = region_offset.clone().unwrap_transparent_reference();
                        let region_length = region_length.clone().unwrap_transparent_reference();

                        let region = Object::OpRegion(OpRegion {
                            space: RegionSpace::from(*region_space),
                            base: region_offset.as_integer()?,
                            length: region_length.as_integer()?,
                            parent_device_path: context.current_scope.clone(),
                        });
                        self.namespace.lock().insert(name.resolve(&context.current_scope)?, region.wrap())?;
                        context.retire_op(op);
                    }
                    Opcode::DataRegion => {
                        let [
                            Argument::Namestring(name),
                            Argument::Object(signature),
                            Argument::Object(oem_id),
                            Argument::Object(oem_table_id),
                        ] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let _signature = signature.as_string()?;
                        let _oem_id = oem_id.as_string()?;
                        let _oem_table_id = oem_table_id.as_string()?;

                        // TODO: once this is integrated into the rest of the crate, load the table
                        log::warn!(
                            "DefDataRegion encountered in AML! We don't actually support these - produced region will be incorrect"
                        );

                        let region = Object::OpRegion(OpRegion {
                            space: RegionSpace::SystemMemory,
                            base: 0,
                            length: 0,
                            parent_device_path: context.current_scope.clone(),
                        });
                        self.namespace.lock().insert(name.resolve(&context.current_scope)?, region.wrap())?;
                        context.retire_op(op);
                    }
                    Opcode::Buffer => {
                        let [
                            Argument::TrackedPc(start_pc),
                            Argument::PkgLength(pkg_length),
                            Argument::Object(buffer_size),
                        ] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let buffer_size = buffer_size.clone().unwrap_transparent_reference().as_integer()?;

                        let buffer_len = context.pkg_remaining(*start_pc, *pkg_length)?;
                        /*
                         * LOGANFY(Fix 2): the initializer must fit BufferSize (stock: slice panic),
                         * and BufferSize is bounded before it is allocated (stock allocated whatever
                         * the AML asked for). ACPICA instead grows the buffer to the initializer; see
                         * the Fix 2 findings.
                         */
                        if buffer_size > MAX_OBJECT_BYTES as u64 {
                            return Err(AmlError::MalformedAml("BufferSize over the interpreter's object limit"));
                        }
                        if buffer_len as u64 > buffer_size {
                            return Err(AmlError::MalformedAml("Buffer initializer longer than its BufferSize"));
                        }
                        let mut buffer = vec![0; buffer_size as usize];
                        buffer[0..buffer_len].copy_from_slice(
                            &context.current_block.stream()
                                [context.current_block.pc..(context.current_block.pc + buffer_len)],
                        );
                        context.current_block.pc += buffer_len;

                        context.contribute_arg(Argument::Object(Object::Buffer(buffer).wrap()));
                        context.retire_op(op);
                    }
                    Opcode::Package => {
                        let mut elements = Vec::with_capacity(op.expected_arguments);
                        for arg in &op.arguments {
                            let Argument::Object(object) = arg else {
                                return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                            };
                            elements.push(object.clone());
                        }

                        /*
                         * We can end up completing a package's in-flight op in two circumstances:
                         *    - If the correct number of elements are supplied, we end up here
                         *      first, and then later in the block's finishing logic.
                         *    - If less elements are supplied, we end up in the block's finishing
                         *      logic to add some `Uninitialized`s, then go round again to complete
                         *      the in-flight operation.
                         *
                         * To make these consistent, we always remove the block here, making sure
                         * we've finished it as a sanity check.
                         */
                        // LOGANFY(Fix 2): stock asserted both. More initializers than NumElements
                        // leaves bytes in the block (ACPICA truncates with a warning; see the Fix 2
                        // findings), and the block must be the package's own.
                        if context.current_block.kind != BlockKind::Package {
                            return Err(AmlError::MalformedAml("Package completed outside its own block"));
                        }
                        if context.peek() != Err(AmlError::RunOutOfStream) {
                            return Err(AmlError::MalformedAml(
                                "Package holds more elements than its NumElements",
                            ));
                        }
                        context.current_block = context
                            .block_stack
                            .pop()
                            .ok_or(AmlError::MalformedAml("Package block without an enclosing block"))?;
                        context.contribute_arg(Argument::Object(Object::Package(elements).wrap()));
                        context.retire_op(op);
                    }
                    Opcode::VarPackage => {
                        // LOGANFY(Fix 2): a VarPackage block can end before its NumElements
                        // (stock: index panic), and NumElements is bounded before it sizes anything.
                        let Some(Argument::Object(total_elements)) = op.arguments.first() else {
                            return Err(AmlError::MalformedAml("VarPackage without its NumElements"));
                        };
                        let total_elements = total_elements.clone().unwrap_transparent_reference().as_integer()?;
                        if total_elements > MAX_OBJECT_BYTES as u64 {
                            return Err(AmlError::MalformedAml("VarPackage over the interpreter's object limit"));
                        }

                        let mut elements = Vec::with_capacity(total_elements as usize);
                        for arg in &op.arguments[1..] {
                            let Argument::Object(object) = arg else {
                                return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                            };
                            elements.push(object.clone());
                        }

                        /*
                         * As above, we always remove the block here after the in-flight op has
                         * been retired.
                         */
                        // LOGANFY(Fix 2): as for `Package` above (stock asserted).
                        if context.current_block.kind != BlockKind::VarPackage {
                            return Err(AmlError::MalformedAml("VarPackage completed outside its own block"));
                        }
                        if context.peek() != Err(AmlError::RunOutOfStream) {
                            return Err(AmlError::MalformedAml(
                                "VarPackage holds more elements than its NumElements",
                            ));
                        }
                        context.current_block = context
                            .block_stack
                            .pop()
                            .ok_or(AmlError::MalformedAml("VarPackage block without an enclosing block"))?;
                        context.contribute_arg(Argument::Object(Object::Package(elements).wrap()));
                        context.retire_op(op);
                    }
                    Opcode::If => {
                        let [
                            Argument::TrackedPc(start_pc),
                            Argument::PkgLength(then_length),
                            Argument::Object(predicate),
                        ] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): `If (Local0)`: the predicate is the Local's value,
                        // not the transparent reference to it (ACPI 6.5 §19.6 `If`; §19.3.5 ASL Data Types).
                        let predicate = predicate.clone().unwrap_transparent_reference().as_integer()?;
                        let remaining_then_length = context.pkg_remaining(*start_pc, *then_length)?;

                        if predicate > 0 {
                            context.start_new_block(BlockKind::IfThenBranch, remaining_then_length)?;
                        } else {
                            context.current_block.pc += remaining_then_length;

                            /*
                             * Skip over the prolog to the else branch if present. Also handle if
                             * there are no more bytes to peek - the `If` op could be the last op
                             * in a block.
                             */
                            const DEF_ELSE_OP: u8 = 0xa1;
                            match context.peek() {
                                Ok(DEF_ELSE_OP) => {
                                    context.next()?;
                                    let _else_length = context.pkglength()?;
                                }
                                Ok(_) => (),
                                Err(AmlError::RunOutOfStream) => (),
                                Err(other) => Err(other)?,
                            }
                        }
                        context.retire_op(op);
                    }
                    opcode @ Opcode::CreateBitField
                    | opcode @ Opcode::CreateByteField
                    | opcode @ Opcode::CreateWordField
                    | opcode @ Opcode::CreateDWordField
                    | opcode @ Opcode::CreateQWordField => {
                        let [Argument::Object(buffer), Argument::Object(index)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let name = context.namestring()?;
                        // LOGANFY(Fix 1, G1): both operands may be Locals/Args. The field is created over
                        // the buffer the Local holds (ACPI 6.5 §19.6 `CreateBitField`...`CreateQWordField`).
                        let buffer = buffer.clone().unwrap_transparent_reference();
                        let index = index.clone().unwrap_transparent_reference().as_integer()?;
                        // LOGANFY(Fix 2): `index * 8` is checked (stock overflowed), and the field is
                        // checked against its buffer (`check_buffer_field`).
                        let bytes = || {
                            index.checked_mul(8).ok_or(AmlError::MalformedAml("buffer field outside its buffer"))
                        };
                        let (offset, length) = match opcode {
                            Opcode::CreateBitField => (index, 1),
                            Opcode::CreateByteField => (bytes()?, 8),
                            Opcode::CreateWordField => (bytes()?, 16),
                            Opcode::CreateDWordField => (bytes()?, 32),
                            Opcode::CreateQWordField => (bytes()?, 64),
                            _ => unreachable!("this arm only matches the Create*Field opcodes"),
                        };
                        let (offset, length) = check_buffer_field(&buffer, offset, length as u64)?;
                        self.namespace.lock().insert(
                            name.resolve(&context.current_scope)?,
                            Object::BufferField { buffer: buffer.clone(), offset, length }.wrap(),
                        )?;
                        context.retire_op(op);
                    }
                    Opcode::CreateField => {
                        let [Argument::Object(buffer), Argument::Object(bit_index), Argument::Object(num_bits)] =
                            &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let name = context.namestring()?;
                        // LOGANFY(Fix 1, G1): as for Create*Field above (ACPI 6.5 §19.6 `CreateField`).
                        let buffer = buffer.clone().unwrap_transparent_reference();
                        let bit_index = bit_index.clone().unwrap_transparent_reference().as_integer()?;
                        let num_bits = num_bits.clone().unwrap_transparent_reference().as_integer()?;
                        // LOGANFY(Fix 2): checked against its buffer, as for Create*Field above.
                        let (offset, length) = check_buffer_field(&buffer, bit_index, num_bits)?;

                        self.namespace.lock().insert(
                            name.resolve(&context.current_scope)?,
                            Object::BufferField { buffer: buffer.clone(), offset, length }.wrap(),
                        )?;
                        context.retire_op(op);
                    }
                    Opcode::Store => {
                        let [Argument::Object(object), target] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        self.do_store(target, object.clone())?;
                        context.retire_op(op);
                    }
                    Opcode::RefOf => {
                        let [Argument::Object(object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let reference =
                            Object::Reference { kind: ReferenceKind::RefOf, inner: object.clone() }.wrap();
                        context.contribute_arg(Argument::Object(reference));
                        context.retire_op(op);
                    }
                    Opcode::CondRefOf => {
                        let [Argument::Object(object), target] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let result = if let Object::Reference { kind: ReferenceKind::Unresolved, .. } = **object {
                            Object::Integer(0)
                        } else {
                            let reference =
                                Object::Reference { kind: ReferenceKind::RefOf, inner: object.clone() }.wrap();
                            self.do_store(target, reference)?;
                            Object::Integer(u64::MAX)
                        };
                        context.contribute_arg(Argument::Object(result.wrap()));
                        context.retire_op(op);
                    }
                    Opcode::DerefOf => {
                        let [Argument::Object(object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        /*
                         * LOGANFY(Fix 1, G2): `Object::typ()` sees through references, so the stock
                         * `typ() == Reference` test was never true and every `DerefOf` of a reference
                         * failed. Match the object itself instead. A Local/Arg holding the reference (or
                         * the name string) is unwrapped first (G1). ACPI 6.5 §19.6 `DerefOf`: the operand
                         * is an ObjectReference (from `RefOf`/`Index`) or a String naming an object; the
                         * result is the referenced object's value, so a referenced field or buffer field
                         * (e.g. `DerefOf (Index (Buffer, i))`, G3) is read.
                         */
                        let object = object.clone().unwrap_transparent_reference();
                        let result = match *object {
                            Object::Reference { kind: ReferenceKind::RefOf | ReferenceKind::Index, ref inner } => {
                                self.read_named_value(inner.clone())?
                            }
                            Object::String(ref name) => {
                                let path = AmlName::from_str(name)?.resolve(&context.current_scope)?;
                                let target = self.namespace.lock().get(path)?.clone();
                                self.read_named_value(target)?
                            }
                            _ => {
                                return Err(AmlError::ObjectNotOfExpectedType {
                                    expected: ObjectType::Reference,
                                    got: object.typ(),
                                });
                            }
                        };
                        context.contribute_arg(Argument::Object(result));
                        context.retire_op(op);
                    }
                    Opcode::Sleep => {
                        let [Argument::Object(msec)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        self.handler.sleep(msec.clone().unwrap_transparent_reference().as_integer()?);
                        context.retire_op(op);
                    }
                    Opcode::Stall => {
                        let [Argument::Object(usec)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        self.handler.stall(usec.clone().unwrap_transparent_reference().as_integer()?);
                        context.retire_op(op);
                    }
                    Opcode::Acquire => {
                        let [Argument::Object(mutex)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let Object::Mutex { mutex, sync_level: _ } = **mutex else {
                            Err(AmlError::InvalidOperationOnObject { op: Operation::Acquire, typ: mutex.typ() })?
                        };
                        let timeout = context.next_u16()?;

                        // TODO: should we do something with the sync level??
                        let acquired = if mutex == self.global_lock_mutex {
                            self.acquire_global_lock(timeout)
                        } else {
                            self.handler.acquire(mutex, timeout)
                        };
                        /*
                         * LOGANFY(Fix 3, 7): `Acquire` returns True (Ones) if the timeout expired and
                         * False (Zero) if the mutex was acquired (ACPI 6.5 §19.6 `Acquire`; ACPICA
                         * `exoparg2.c`: AE_TIME becomes a TRUE result, not an error). Stock contributed
                         * nothing, so `Local0 = Acquire (...)` left Local0 Uninitialized.
                         */
                        let timed_out = match acquired {
                            Ok(()) => false,
                            Err(AmlError::MutexAcquireTimeout) => true,
                            Err(other) => return Err(other),
                        };
                        let result = if timed_out { u64::MAX } else { 0 };
                        context.contribute_arg(Argument::Object(Object::Integer(result).wrap()));
                        context.retire_op(op);
                    }
                    Opcode::Release => {
                        let [Argument::Object(mutex)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let Object::Mutex { mutex, sync_level: _ } = **mutex else {
                            Err(AmlError::InvalidOperationOnObject { op: Operation::Release, typ: mutex.typ() })?
                        };

                        // TODO: should we do something with the sync level??
                        if mutex == self.global_lock_mutex {
                            self.release_global_lock()?;
                        } else {
                            self.handler.release(mutex);
                        }

                        context.retire_op(op);
                    }
                    Opcode::InternalMethodCall => {
                        let [Argument::Object(method), Argument::Namestring(method_scope)] = &op.arguments[0..2]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };

                        // LOGANFY(Fix 2): stock panicked on a non-object argument.
                        let args = op.arguments[2..]
                            .iter()
                            .map(|arg| {
                                if let Argument::Object(arg) = arg {
                                    Ok(arg.clone())
                                } else {
                                    Err(AmlError::MalformedAml("operator arguments of an unexpected kind"))
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        if let Object::Method { .. } = **method {
                            // LOGANFY(Fix 2): bounded method call depth (see MAX_NESTING).
                            if self.context_stack.lock().len() >= MAX_NESTING {
                                return Err(AmlError::MalformedAml(
                                    "method calls nested deeper than the interpreter's limit",
                                ));
                            }
                            self.namespace
                                .lock()
                                .add_level(method_scope.clone(), NamespaceLevelKind::MethodLocals)?;

                            let new_context =
                                MethodContext::new_from_method(method.clone(), args, method_scope.clone())?;
                            let old_context = mem::replace(context, new_context);
                            self.context_stack.lock().push(old_context);
                            context.retire_op(op);
                        } else if let Object::NativeMethod { ref f, .. } = **method {
                            let result = f(&args)?;
                            context.contribute_arg(Argument::Object(result));
                        } else {
                            // LOGANFY(Fix 2): stock `panic!()`.
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        }
                    }
                    Opcode::Return => {
                        let [Argument::Object(object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        let object = object.clone().unwrap_transparent_reference();

                        // LOGANFY(Fix 1, b): only a context this execution pushed is its caller.
                        let caller = self.pop_caller_context(base);
                        if let Some(last) = caller {
                            *context = last;
                            context.contribute_arg(Argument::Object(object.clone()));
                            context.retire_op(op);
                        } else {
                            /*
                             * If this is the top-most context, this is a `Return` from the actual
                             * method.
                             */
                            return Ok(object.clone());
                        }
                    }
                    Opcode::ObjectType => {
                        let [Argument::Object(object)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // TODO: this should technically support scopes as well - this is less easy
                        // (they should return `0`)
                        let typ = match object.typ() {
                            ObjectType::Uninitialized => 0,
                            ObjectType::Integer => 1,
                            ObjectType::String => 2,
                            ObjectType::Buffer => 3,
                            ObjectType::Package => 4,
                            ObjectType::FieldUnit => 5,
                            ObjectType::Device => 6,
                            ObjectType::Event => 7,
                            ObjectType::Method => 8,
                            ObjectType::Mutex => 9,
                            ObjectType::OpRegion => 10,
                            ObjectType::PowerResource => 11,
                            ObjectType::Processor => 12,
                            ObjectType::ThermalZone => 13,
                            ObjectType::BufferField => 14,
                            // XXX: 15 is reserved
                            ObjectType::Debug => 16,
                            // LOGANFY(Fix 2): unreachable: `Object::typ()` recurses through
                            // references, so it never returns `Reference`.
                            ObjectType::Reference => unreachable!("typ() never returns Reference"),
                            // LOGANFY(Fix 2): stock `todo!()`; the interpreter never builds a
                            // RawDataBuffer and `typ()` maps nothing else to it.
                            ObjectType::RawDataBuffer => unreachable!("no RawDataBuffer is ever constructed"),
                        };

                        context.contribute_arg(Argument::Object(Object::Integer(typ).wrap()));
                        context.retire_op(op);
                    }
                    Opcode::SizeOf => self.do_size_of(context, op)?,
                    Opcode::Index => self.do_index(context, op)?,
                    Opcode::BankField => {
                        let [
                            Argument::TrackedPc(start_pc),
                            Argument::PkgLength(pkg_length),
                            Argument::Namestring(region_name),
                            Argument::Namestring(bank_name),
                            Argument::Object(bank_value),
                        ] = &op.arguments[..]
                        else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
                        let bank_value = bank_value.clone().unwrap_transparent_reference().as_integer()?;
                        let field_flags = context.next()?;

                        let (region, bank) = {
                            let namespace = self.namespace.lock();
                            let (_, region) = namespace.search(region_name, &context.current_scope)?;
                            let (_, bank) = namespace.search(bank_name, &context.current_scope)?;
                            (region, bank)
                        };

                        let kind = FieldUnitKind::Bank { region, bank, bank_value };
                        self.parse_field_list(context, kind, *start_pc, *pkg_length, field_flags)?;
                        context.retire_op(op);
                    }
                    Opcode::While => {
                        /*
                         * We've just evaluated the predicate for an iteration of a while loop. If
                         * false, skip over the rest of the loop, otherwise carry on.
                         */
                        let [Argument::Object(predicate)] = &op.arguments[..] else {
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        };
                        // LOGANFY(Fix 1, G1): as for `If` (ACPI 6.5 §19.6 `While`).
                        let predicate = predicate.clone().unwrap_transparent_reference().as_integer()?;

                        if predicate == 0 {
                            // Exit from the while loop by skipping out of the current block
                            context.current_block = context
                                .block_stack
                                .pop()
                                .ok_or(AmlError::MalformedAml("block structure out of step with the stream"))?;
                            context.retire_op(op);
                        }
                    }
                    // LOGANFY(Fix 2): an opcode with an in-flight op but no completion here (stock:
                    // panic). `Notify` is the one AML reaches; it is OT-86 L4.
                    other => return Err(fail_closed(&alloc::format!("{other:?}"))),
                }
            }

            /*
             * Now that we've retired as many in-flight operations as we have arguments for, move
             * forward in the AML stream.
             */
            let opcode = match context.opcode() {
                Ok(opcode) => opcode,
                Err(AmlError::RunOutOfStream) => {
                    /*
                     * We've reached the end of the current block. What we should do about this
                     * depends on what type of block it was.
                     */
                    match &context.current_block.kind {
                        BlockKind::Table => {
                            break Ok(Object::Uninitialized.wrap());
                        }
                        BlockKind::Method { method_scope } => {
                            self.namespace.lock().remove_level(method_scope.clone())?;

                            // LOGANFY(Fix 1, b): as for `Return`.
                            let caller = self.pop_caller_context(base);
                            if let Some(prev_context) = caller {
                                *context = prev_context;
                                continue;
                            } else {
                                /*
                                 * If there is no explicit `Return` op, the result is undefined. We
                                 * just return an uninitialized object.
                                 */
                                return Ok(Object::Uninitialized.wrap());
                            }
                        }
                        BlockKind::Scope { old_scope } => {
                            let old_scope = old_scope.clone();
                            // LOGANFY(Fix 2): stock asserted.
                            if context.block_stack.is_empty() {
                                return Err(AmlError::MalformedAml("block structure out of step with the stream"));
                            }
                            context.current_block = context
                                .block_stack
                                .pop()
                                .ok_or(AmlError::MalformedAml("block structure out of step with the stream"))?;
                            context.current_scope = old_scope;
                            // Go round the loop again to get the next opcode for the new block
                            continue;
                        }
                        BlockKind::Package => {
                            /*
                             * We've reached the end of the package. The in-flight op may have
                             * already been completed in the case of the package specifying all of
                             * its elements, or reach the end of the block here if it does not.
                             *
                             * In the latter case, fill in the rest of the package with
                             * *distinct* uninitialized objects, and go round again to complete the
                             * in-flight op.
                             */
                            // LOGANFY(Fix 2): stock asserted.
                            if context.block_stack.is_empty() {
                                return Err(AmlError::MalformedAml("block structure out of step with the stream"));
                            }

                            if let Some(package_op) = context.in_flight.last_mut()
                                && package_op.op == Opcode::Package
                            {
                                let num_elements_left = package_op.expected_arguments - package_op.arguments.len();
                                for _ in 0..num_elements_left {
                                    package_op.arguments.push(Argument::Object(Object::Uninitialized.wrap()));
                                }
                            }

                            // XXX: don't remove the package's block. Refer to completion of
                            // package ops for rationale here.
                            continue;
                        }
                        BlockKind::VarPackage => {
                            if context.block_stack.is_empty() {
                                return Err(AmlError::MalformedAml("VarPackage block without an enclosing block"));
                            }

                            if let Some(package_op) = context.in_flight.last_mut()
                                && package_op.op == Opcode::VarPackage
                            {
                                /*
                                 * LOGANFY(Fix 2): stock indexed `arguments[0]` (absent when the block
                                 * ends before NumElements), subtracted unchecked (more initializers
                                 * than NumElements underflowed) and pushed NumElements placeholders
                                 * unbounded.
                                 */
                                let num_elements_left = {
                                    let Some(Argument::Object(total_elements)) = package_op.arguments.first()
                                    else {
                                        return Err(AmlError::MalformedAml("VarPackage without its NumElements"));
                                    };
                                    let total_elements =
                                        total_elements.clone().unwrap_transparent_reference().as_integer()?;
                                    if total_elements > MAX_OBJECT_BYTES as u64 {
                                        return Err(AmlError::MalformedAml(
                                            "VarPackage over the interpreter's object limit",
                                        ));
                                    }

                                    // Update the expected number of arguments to terminate the in-flight op
                                    package_op.expected_arguments = package_op.arguments.len();
                                    (total_elements as usize).checked_sub(package_op.arguments.len() - 1).ok_or(
                                        AmlError::MalformedAml(
                                            "VarPackage holds more elements than its NumElements",
                                        ),
                                    )?
                                };

                                for _ in 0..num_elements_left {
                                    package_op.arguments.push(Argument::Object(Object::Uninitialized.wrap()));
                                }
                            }

                            // As above, leave the package's block.
                            continue;
                        }
                        BlockKind::IfThenBranch => {
                            context.current_block = context
                                .block_stack
                                .pop()
                                .ok_or(AmlError::MalformedAml("block structure out of step with the stream"))?;

                            /*
                             * Check for an else-branch, and skip over it. We need to handle the
                             * case here where there isn't a next byte - that just means the `If`
                             * is the last op in a block.
                             */
                            const DEF_ELSE_OP: u8 = 0xa1;
                            match context.peek() {
                                Ok(DEF_ELSE_OP) => {
                                    context.next()?;
                                    let start_pc = context.current_block.pc;
                                    let else_length = context.pkglength()?;
                                    context.current_block.pc += context.pkg_remaining(start_pc, else_length)?;
                                }
                                Ok(_) => (),
                                Err(AmlError::RunOutOfStream) => (),
                                Err(other) => Err(other)?,
                            };

                            continue;
                        }
                        BlockKind::While { start_pc } => {
                            /*
                             * LOGANFY(Fix 2): a While block with no predicate bytes ends with its
                             * predicate op still empty; stock went round again and pushed another,
                             * so the in-flight stack grew without bound.
                             */
                            if context
                                .in_flight
                                .last()
                                .is_some_and(|op| op.op == Opcode::While && op.arguments.is_empty())
                            {
                                return Err(AmlError::MalformedAml("While without a predicate"));
                            }
                            /*
                             * Go round again, and create a new in-flight op to have a look at the
                             * predicate.
                             */
                            context.current_block.pc = *start_pc;
                            context.start(OpInFlight::new(Opcode::While, &[ResolveBehaviour::TermArg]));
                            continue;
                        }
                    }
                }
                Err(other_err) => return Err(other_err),
            };
            match opcode {
                Opcode::Zero => {
                    /*
                     * This represents a `Zero` operand that should create an `Integer` object in
                     * most places, but could also encode a `NullName` if we are expecting a
                     * `Target`.
                     */
                    if context.last_op()?.resolve_behaviour()? == ResolveBehaviour::Target {
                        context.last_op()?.arguments.push(Argument::Null);
                    } else {
                        context.last_op()?.arguments.push(Argument::Object(Object::Integer(0).wrap()));
                    }
                }
                Opcode::One => {
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(1).wrap()));
                }
                Opcode::Alias => {
                    let source = context.namestring()?;
                    let alias = context.namestring()?;

                    let mut namespace = self.namespace.lock();
                    let object = namespace.get(source.resolve(&context.current_scope)?)?.clone();
                    let alias = alias.resolve(&context.current_scope)?;
                    namespace.create_alias(alias, object)?;
                }
                Opcode::Name => {
                    let name = context.namestring()?;
                    context.start(OpInFlight::new_with(
                        Opcode::Name,
                        vec![Argument::Namestring(name)],
                        &[ResolveBehaviour::Placeholder, ResolveBehaviour::TermArg],
                    ));
                }
                Opcode::BytePrefix => {
                    let value = context.next()?;
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(value as u64).wrap()));
                }
                Opcode::WordPrefix => {
                    let value = context.next_u16()?;
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(value as u64).wrap()));
                }
                Opcode::DWordPrefix => {
                    let value = context.next_u32()?;
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(value as u64).wrap()));
                }
                Opcode::StringPrefix => {
                    let str_start = context.current_block.pc;
                    while context.next()? != b'\0' {}
                    // TODO: handle err
                    // LOGANFY(Fix 2): stock `unwrap()`ed; AML strings are ASCII (ACPI 6.5 §20.2.3).
                    let str = String::from(
                        str::from_utf8(&context.current_block.stream()[str_start..(context.current_block.pc - 1)])
                            .map_err(|_| AmlError::MalformedAml("String literal that is not ASCII/UTF-8"))?,
                    );
                    context.last_op()?.arguments.push(Argument::Object(Object::String(str).wrap()));
                }
                Opcode::QWordPrefix => {
                    let value = context.next_u64()?;
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(value).wrap()));
                }
                Opcode::Scope => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let name = context.namestring()?;

                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    let new_scope = name.resolve(&context.current_scope)?;
                    self.namespace.lock().add_level(new_scope.clone(), NamespaceLevelKind::Scope)?;

                    let old_scope = mem::replace(&mut context.current_scope, new_scope);
                    context.start_new_block(BlockKind::Scope { old_scope }, remaining_length)?;
                }
                Opcode::Buffer => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    context.start(OpInFlight::new_with(
                        Opcode::Buffer,
                        vec![Argument::TrackedPc(start_pc), Argument::PkgLength(pkg_length)],
                        &[ResolveBehaviour::Placeholder, ResolveBehaviour::Placeholder, ResolveBehaviour::TermArg],
                    ));
                }
                Opcode::Package => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let num_elements = context.next()?;

                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    /*
                     * We now need to interpret an arbitrary number of package elements, bounded by
                     * the remaining pkglength. This may be less than `num_elements` - the
                     * remaining elements of the package are uninitialized. We utilise a
                     * combination of a block to manage the pkglength, plus an in-flight op to
                     * store interpreted arguments.
                     */
                    context.start(OpInFlight::new_dynamic(
                        Opcode::Package,
                        num_elements as usize,
                        &[ResolveBehaviour::AsPackageElements],
                    ));
                    context.start_new_block(BlockKind::Package, remaining_length)?;
                }
                Opcode::VarPackage => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    /*
                     * For variable packages, we're first going to parse a `TermArg` that encodes,
                     * dynamically, how many elements the package will have. We then accept as many
                     * elements as remain in the block, and we'll sort out how many are supposed to
                     * be in the package later.
                     */
                    context.start(OpInFlight::new_dynamic(
                        Opcode::VarPackage,
                        usize::MAX,
                        &[ResolveBehaviour::TermArg, ResolveBehaviour::AsPackageElements],
                    ));
                    context.start_new_block(BlockKind::VarPackage, remaining_length)?;
                }
                Opcode::Method => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let name = context.namestring()?;
                    let flags = MethodFlags(context.next()?);

                    let code_len = context.pkg_remaining(start_pc, pkg_length)?;
                    let code = context.current_block.stream()
                        [context.current_block.pc..(context.current_block.pc + code_len)]
                        .to_vec();
                    context.current_block.pc += code_len;

                    let name = name.resolve(&context.current_scope)?;
                    self.namespace.lock().insert(name, Object::Method { code, flags }.wrap())?;
                }
                Opcode::External => {
                    let _name = context.namestring()?;
                    let _object_type = context.next()?;
                    let _arg_count = context.next()?;
                }
                Opcode::Mutex => {
                    let name = context.namestring()?;
                    let sync_level = context.next()?;

                    let name = name.resolve(&context.current_scope)?;
                    let mutex = self.handler.create_mutex();
                    self.namespace.lock().insert(name, Object::Mutex { mutex, sync_level }.wrap())?;
                }
                Opcode::Event => {
                    let name = context.namestring()?;

                    let name = name.resolve(&context.current_scope)?;
                    self.namespace.lock().insert(name, Object::Event(Arc::new(AtomicU64::new(0))).wrap())?;
                }
                // LOGANFY(Fix 2): not built (OT-86 L4); fail closed instead of `todo!()`.
                Opcode::LoadTable => return Err(fail_closed("LoadTable")),
                Opcode::Load => return Err(fail_closed("Load")),
                Opcode::Stall => context.start(OpInFlight::new(Opcode::Stall, &[ResolveBehaviour::TermArg])),
                Opcode::Sleep => context.start(OpInFlight::new(Opcode::Sleep, &[ResolveBehaviour::TermArg])),
                Opcode::Acquire => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                Opcode::Release => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                Opcode::Signal => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                Opcode::Wait => context
                    .start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName, ResolveBehaviour::TermArg])),
                Opcode::Reset => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                Opcode::Notify => context
                    .start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName, ResolveBehaviour::TermArg])),
                Opcode::FromBCD | Opcode::ToBCD => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg, ResolveBehaviour::Target]))
                }
                Opcode::Revision => {
                    context.contribute_arg(Argument::Object(Object::Integer(INTERPRETER_REVISION).wrap()));
                }
                Opcode::Debug => {
                    context.contribute_arg(Argument::Object(Object::Debug.wrap()));
                }
                Opcode::Fatal => {
                    let typ = context.next()?;
                    let code = context.next_u32()?;
                    context.start(OpInFlight::new_with(
                        Opcode::Fatal,
                        vec![Argument::ByteData(typ), Argument::DWordData(code)],
                        &[ResolveBehaviour::Placeholder, ResolveBehaviour::Placeholder, ResolveBehaviour::TermArg],
                    ));
                }
                Opcode::Timer => {
                    // Time has to be monotonically-increasing, in 100ns units
                    let time = self.handler.nanos_since_boot() / 100;
                    context.contribute_arg(Argument::Object(Object::Integer(time).wrap()));
                }
                Opcode::OpRegion => {
                    let name = context.namestring()?;
                    let region_space = context.next()?;
                    context.start(OpInFlight::new_with(
                        Opcode::OpRegion,
                        vec![Argument::Namestring(name), Argument::ByteData(region_space)],
                        &[
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::TermArg,
                            ResolveBehaviour::TermArg,
                        ],
                    ));
                }
                Opcode::DataRegion => {
                    let name = context.namestring()?;
                    context.start(OpInFlight::new_with(
                        Opcode::DataRegion,
                        vec![Argument::Namestring(name)],
                        &[
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::TermArg,
                            ResolveBehaviour::TermArg,
                            ResolveBehaviour::TermArg,
                        ],
                    ));
                }
                Opcode::Field => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let region_name = context.namestring()?;
                    let field_flags = context.next()?;

                    let region = self.namespace.lock().get(region_name.resolve(&context.current_scope)?)?.clone();
                    let kind = FieldUnitKind::Normal { region };
                    self.parse_field_list(context, kind, start_pc, pkg_length, field_flags)?;
                }
                Opcode::BankField => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let region_name = context.namestring()?;
                    let bank_name = context.namestring()?;

                    context.start(OpInFlight::new_with(
                        Opcode::BankField,
                        vec![
                            Argument::TrackedPc(start_pc),
                            Argument::PkgLength(pkg_length),
                            Argument::Namestring(region_name),
                            Argument::Namestring(bank_name),
                        ],
                        &[
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::Placeholder,
                            ResolveBehaviour::TermArg,
                        ],
                    ));
                }
                Opcode::IndexField => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let index_name = context.namestring()?;
                    let data_name = context.namestring()?;
                    let field_flags = context.next()?;

                    let (index, data) = {
                        let namespace = self.namespace.lock();
                        let (_, index) = namespace.search(&index_name, &context.current_scope)?;
                        let (_, data) = namespace.search(&data_name, &context.current_scope)?;
                        (index, data)
                    };
                    let kind = FieldUnitKind::Index { index, data };
                    self.parse_field_list(context, kind, start_pc, pkg_length, field_flags)?;
                }
                Opcode::Device | Opcode::ThermalZone => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let name = context.namestring()?;

                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    let new_scope = name.resolve(&context.current_scope)?;
                    let (kind, object) = match opcode {
                        Opcode::Device => (NamespaceLevelKind::Device, Object::Device),
                        Opcode::ThermalZone => (NamespaceLevelKind::ThermalZone, Object::ThermalZone),
                        _ => unreachable!(),
                    };
                    let mut namespace = self.namespace.lock();
                    namespace.add_level(new_scope.clone(), kind)?;
                    namespace.insert(new_scope.clone(), object.wrap())?;

                    let old_scope = mem::replace(&mut context.current_scope, new_scope);
                    context.start_new_block(BlockKind::Scope { old_scope }, remaining_length)?;
                }
                Opcode::Processor => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let name = context.namestring()?;
                    let proc_id = context.next()?;
                    let pblk_address = context.next_u32()?;
                    let pblk_length = context.next()?;

                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    let new_scope = name.resolve(&context.current_scope)?;
                    let object = Object::Processor { proc_id, pblk_address, pblk_length };
                    let mut namespace = self.namespace.lock();
                    namespace.add_level(new_scope.clone(), NamespaceLevelKind::Processor)?;
                    namespace.insert(new_scope.clone(), object.wrap())?;

                    let old_scope = mem::replace(&mut context.current_scope, new_scope);
                    context.start_new_block(BlockKind::Scope { old_scope }, remaining_length)?;
                }
                Opcode::PowerRes => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let name = context.namestring()?;
                    let system_level = context.next()?;
                    let resource_order = context.next_u16()?;

                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;

                    let new_scope = name.resolve(&context.current_scope)?;
                    let object = Object::PowerResource { system_level, resource_order };
                    let mut namespace = self.namespace.lock();
                    namespace.add_level(new_scope.clone(), NamespaceLevelKind::PowerResource)?;
                    namespace.insert(new_scope.clone(), object.wrap())?;

                    let old_scope = mem::replace(&mut context.current_scope, new_scope);
                    context.start_new_block(BlockKind::Scope { old_scope }, remaining_length)?;
                }
                Opcode::Local(local) => {
                    let local = context.locals[local as usize].clone();
                    context.last_op()?.arguments.push(Argument::Object(
                        Object::Reference { kind: ReferenceKind::LocalOrArg, inner: local }.wrap(),
                    ));
                }
                Opcode::Arg(arg) => {
                    let arg = context.args[arg as usize].clone();
                    context.last_op()?.arguments.push(Argument::Object(
                        Object::Reference { kind: ReferenceKind::LocalOrArg, inner: arg }.wrap(),
                    ));
                }
                Opcode::Store => context.start(OpInFlight::new(
                    Opcode::Store,
                    &[ResolveBehaviour::TermArg, ResolveBehaviour::SuperName],
                )),
                Opcode::RefOf => context.start(OpInFlight::new(Opcode::RefOf, &[ResolveBehaviour::SuperName])),
                Opcode::CondRefOf => context.start(OpInFlight::new(
                    opcode,
                    &[ResolveBehaviour::SuperNameIfExists, ResolveBehaviour::Target],
                )),

                Opcode::DualNamePrefix
                | Opcode::MultiNamePrefix
                | Opcode::Digit(_)
                | Opcode::NameChar(_)
                | Opcode::RootChar
                | Opcode::ParentPrefixChar => {
                    context.current_block.pc -= 1;
                    let name = context.namestring()?;

                    let behaviour = context
                        .in_flight
                        .last()
                        .map(|op| op.resolve_behaviour())
                        .transpose()?
                        .unwrap_or(ResolveBehaviour::TermArg);
                    match behaviour {
                        // XXX: `NullName` is handled separately given its ambiguity with `Zero`
                        ResolveBehaviour::SuperName | ResolveBehaviour::Target => {
                            let object = self.namespace.lock().search(&name, &context.current_scope);
                            match object {
                                Ok((_resolved_name, object)) => {
                                    context.last_op()?.arguments.push(Argument::Object(object));
                                }
                                Err(err) => Err(err)?,
                            }
                        }
                        ResolveBehaviour::SuperNameIfExists => {
                            let object = self.namespace.lock().search(&name, &context.current_scope);
                            match object {
                                Ok((_resolved_name, object)) => {
                                    context.last_op()?.arguments.push(Argument::Object(object));
                                }
                                Err(AmlError::ObjectDoesNotExist(_)) => {
                                    let reference = Object::Reference {
                                        kind: ReferenceKind::Unresolved,
                                        inner: Object::String(name.to_string()).wrap(),
                                    };
                                    context.last_op()?.arguments.push(Argument::Object(reference.wrap()));
                                }
                                Err(err) => Err(err)?,
                            }
                        }
                        ResolveBehaviour::TermArg => {
                            let object = self.namespace.lock().search(&name, &context.current_scope);
                            match object {
                                Ok((resolved_name, object)) => {
                                    if let Object::Method { flags, .. } | Object::NativeMethod { flags, .. } =
                                        *object
                                    {
                                        context.start(OpInFlight::new_with_dynamic(
                                            Opcode::InternalMethodCall,
                                            vec![Argument::Object(object), Argument::Namestring(resolved_name)],
                                            flags.arg_count(),
                                            &[
                                                ResolveBehaviour::Placeholder,
                                                ResolveBehaviour::Placeholder,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                                ResolveBehaviour::TermArg,
                                            ],
                                        ))
                                    } else if let Object::FieldUnit(ref field) = *object {
                                        let value = self.do_field_read(field)?;
                                        context.last_op()?.arguments.push(Argument::Object(value));
                                    } else if let Object::BufferField { .. } = *object {
                                        // LOGANFY(Fix 1, G3): a buffer field named as a TermArg is read,
                                        // exactly like a field unit above.
                                        let value = self.read_named_value(object.clone())?;
                                        context.last_op()?.arguments.push(Argument::Object(value));
                                    } else {
                                        context.last_op()?.arguments.push(Argument::Object(object));
                                    }
                                }
                                Err(err) => Err(err)?,
                            }
                        }
                        ResolveBehaviour::AsPackageElements => {
                            context
                                .last_op()?
                                .arguments
                                .push(Argument::Object(Object::String(name.to_string()).wrap()));
                        }
                        ResolveBehaviour::Placeholder => {
                            // LOGANFY(Fix 2): stock `panic!`.
                            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
                        }
                    }
                }

                Opcode::Add
                | Opcode::Subtract
                | Opcode::Multiply
                | Opcode::ShiftLeft
                | Opcode::ShiftRight
                | Opcode::Mod
                | Opcode::Nand
                | Opcode::And
                | Opcode::Or
                | Opcode::Nor
                | Opcode::Xor
                | Opcode::Concat => {
                    context.start(OpInFlight::new(
                        opcode,
                        &[ResolveBehaviour::TermArg, ResolveBehaviour::TermArg, ResolveBehaviour::Target],
                    ));
                }

                Opcode::Divide => context.start(OpInFlight::new(
                    Opcode::Divide,
                    &[
                        ResolveBehaviour::TermArg,
                        ResolveBehaviour::TermArg,
                        ResolveBehaviour::Target,
                        ResolveBehaviour::Target,
                    ],
                )),
                Opcode::Increment | Opcode::Decrement => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName]))
                }
                Opcode::Not => context
                    .start(OpInFlight::new(Opcode::Not, &[ResolveBehaviour::TermArg, ResolveBehaviour::Target])),
                Opcode::FindSetLeftBit | Opcode::FindSetRightBit => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg, ResolveBehaviour::Target]))
                }
                Opcode::DerefOf => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg])),
                Opcode::ConcatRes => context.start(OpInFlight::new(
                    opcode,
                    &[ResolveBehaviour::TermArg, ResolveBehaviour::TermArg, ResolveBehaviour::Target],
                )),
                Opcode::SizeOf => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                Opcode::Index => context.start(OpInFlight::new(
                    opcode,
                    &[ResolveBehaviour::TermArg, ResolveBehaviour::TermArg, ResolveBehaviour::Target],
                )),
                /*
                 * TODO
                 * Match is a difficult opcode to parse, as it interleaves dynamic arguments and
                 * random bytes that need to be extracted as you go. I think we'll need to use 1+
                 * internal in-flight ops to parse the static bytedatas as we go, and then retire
                 * the real op at the end.
                 */
                // LOGANFY(Fix 2): fail closed instead of `todo!()` (OT-86 L4).
                Opcode::Match => return Err(fail_closed("Match")),

                Opcode::CreateBitField
                | Opcode::CreateByteField
                | Opcode::CreateWordField
                | Opcode::CreateDWordField
                | Opcode::CreateQWordField => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg; 2]))
                }
                Opcode::CreateField => {
                    context.start(OpInFlight::new(Opcode::CreateField, &[ResolveBehaviour::TermArg; 3]))
                }

                /*
                 * LOGANFY(Fix 3): stock started `LNot` in this arm with two operands, so `!X`
                 * swallowed the next term as a second operand: `If (LNot (One)) { Return (One) }`
                 * executed the `Return`. `DefLNot := LNotOp Operand` (ACPI 6.5 §20.2.5.4; ACPICA
                 * `acopcode.h` `ARGP_LNOT_OP ARGP_LIST1 (ARGP_TERMARG)`). The 0x92-prefixed
                 * `LNotEqual`, `LLessEqual` and `LGreaterEqual` keep two.
                 */
                Opcode::LNot => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg])),
                Opcode::LAnd
                | Opcode::LOr
                | Opcode::LNotEqual
                | Opcode::LLessEqual
                | Opcode::LGreaterEqual
                | Opcode::LEqual
                | Opcode::LGreater
                | Opcode::LLess => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg; 2]));
                }

                Opcode::ToBuffer | Opcode::ToDecimalString | Opcode::ToHexString | Opcode::ToInteger => {
                    context.start(OpInFlight::new(opcode, &[ResolveBehaviour::TermArg, ResolveBehaviour::Target]))
                }
                Opcode::ToString => context.start(OpInFlight::new(
                    opcode,
                    &[ResolveBehaviour::TermArg, ResolveBehaviour::TermArg, ResolveBehaviour::Target],
                )),

                Opcode::ObjectType => context.start(OpInFlight::new(opcode, &[ResolveBehaviour::SuperName])),
                // LOGANFY(Fix 2): fail closed instead of `todo!()` (OT-86 L4).
                Opcode::CopyObject => return Err(fail_closed("CopyObject")),
                Opcode::Mid => context.start(OpInFlight::new(
                    Opcode::Mid,
                    &[
                        ResolveBehaviour::TermArg,
                        ResolveBehaviour::TermArg,
                        ResolveBehaviour::TermArg,
                        ResolveBehaviour::Target,
                    ],
                )),
                Opcode::If => {
                    let start_pc = context.current_block.pc;
                    let then_length = context.pkglength()?;
                    context.start(OpInFlight::new_with(
                        Opcode::If,
                        vec![Argument::TrackedPc(start_pc), Argument::PkgLength(then_length)],
                        &[ResolveBehaviour::Placeholder, ResolveBehaviour::Placeholder, ResolveBehaviour::TermArg],
                    ));
                }
                Opcode::Else => return Err(AmlError::ElseFoundWithoutCorrespondingIf),
                Opcode::While => {
                    let start_pc = context.current_block.pc;
                    let pkg_length = context.pkglength()?;
                    let remaining_length = context.pkg_remaining(start_pc, pkg_length)?;
                    context.start_new_block(
                        BlockKind::While { start_pc: context.current_block.pc },
                        remaining_length,
                    )?;
                    context.start(OpInFlight::new(Opcode::While, &[ResolveBehaviour::TermArg]));
                }
                Opcode::Continue => {
                    if let BlockKind::While { start_pc } = &context.current_block.kind {
                        context.current_block.pc = *start_pc;
                    } else {
                        loop {
                            let Some(block) = context.block_stack.pop() else {
                                Err(AmlError::ContinueOutsideOfWhile)?
                            };
                            if let BlockKind::While { start_pc } = block.kind {
                                context.current_block.pc = start_pc;
                                break;
                            }
                        }
                    }
                    context.start(OpInFlight::new(Opcode::While, &[ResolveBehaviour::TermArg]));
                }
                Opcode::Break => {
                    if let BlockKind::While { .. } = &context.current_block.kind {
                        context.current_block = context
                            .block_stack
                            .pop()
                            .ok_or(AmlError::MalformedAml("block structure out of step with the stream"))?;
                    } else {
                        loop {
                            let Some(block) = context.block_stack.pop() else {
                                Err(AmlError::BreakOutsideOfWhile)?
                            };
                            if let BlockKind::While { .. } = block.kind {
                                context.current_block = context.block_stack.pop().ok_or(
                                    AmlError::MalformedAml("block structure out of step with the stream"),
                                )?;
                                break;
                            }
                        }
                    }
                }
                Opcode::Return => context.start(OpInFlight::new(Opcode::Return, &[ResolveBehaviour::TermArg])),
                Opcode::Noop => {}
                Opcode::Breakpoint => {
                    self.handler.breakpoint();
                }
                Opcode::Ones => {
                    context.last_op()?.arguments.push(Argument::Object(Object::Integer(u64::MAX).wrap()));
                }

                // LOGANFY(Fix 2): unreachable: `opcode()` never decodes this internal opcode
                // from the byte stream.
                Opcode::InternalMethodCall => unreachable!("InternalMethodCall is never decoded"),
            }
        }
    }

    /// LOGANFY(Fix 1, b): pop the caller context of the method that is returning, if this
    /// execution pushed it (the stack is deeper than `base`, its depth on entry).
    fn pop_caller_context(&self, base: usize) -> Option<MethodContext> {
        let mut stack = self.context_stack.lock();
        if stack.len() > base { stack.pop() } else { None }
    }

    fn parse_field_list(
        &self,
        context: &mut MethodContext,
        kind: FieldUnitKind,
        start_pc: usize,
        pkg_length: usize,
        flags: u8,
    ) -> Result<(), AmlError> {
        const RESERVED_FIELD: u8 = 0x00;
        const ACCESS_FIELD: u8 = 0x01;
        const CONNECT_FIELD: u8 = 0x02;
        const EXTENDED_ACCESS_FIELD: u8 = 0x03;

        let mut field_offset = 0;
        let mut access_unimplemented = false;
        let mut flags = flags;

        while context.current_block.pc < (start_pc + pkg_length) {
            match context.next()? {
                RESERVED_FIELD => {
                    let length = context.pkglength()?;
                    field_offset += length;
                }
                ACCESS_FIELD => {
                    /*
                     * These aren't actually fields themselves, but are created by `AccessAs` AML
                     * elements. They change the access type and attributes for remaining fields in
                     * the list.
                     */
                    let access_type = context.next()?;
                    let _access_attrib = context.next()?;
                    /*
                     * LOGANFY(Fix 3, 10): `AccessField := 0x01 AccessType AccessAttrib` (ACPI 6.5
                     * §20.2.5.2). The access type applies to every later field in the list, as
                     * ACPICA `dsfield.c` `AcpiDsGetFieldNames` does: the flags keep their other
                     * bits and take `AccessType & 0x07`. The attribute is meaningful only to
                     * serial-bus regions, which the crate does not access (their fields follow a
                     * `Connection` and fail closed). Fix 2 parsed it and failed later fields
                     * closed; stock `todo!()`.
                     */
                    flags = (flags & !0x0f) | (access_type & 0x07);
                }
                CONNECT_FIELD => {
                    /*
                     * LOGANFY(Fix 1, c): parse only. `ConnectField := 0x02 NameString | 0x02
                     * BufferData` (ACPI 6.5 §20.2.5.2 Named Objects Encoding; §19.6 `Connection`),
                     * where BufferData is a `DefBuffer` (`BufferOp PkgLength BufferSize ByteList`).
                     * Its resource is not decoded or used: the fields after it are marked and fail
                     * closed on access, so no value is ever made up for them.
                     */
                    const BUFFER_OP: u8 = 0x11;
                    if context.peek()? == BUFFER_OP {
                        context.next()?;
                        let buffer_start = context.current_block.pc;
                        let buffer_length = context.pkglength()?;
                        context.current_block.pc = buffer_start + buffer_length;
                    } else {
                        let _resource_name = context.namestring()?;
                    }
                    access_unimplemented = true;
                }
                EXTENDED_ACCESS_FIELD => {
                    /*
                     * LOGANFY(Fix 1, c): parse only, as for `Connection` above.
                     * `ExtendedAccessField := 0x03 AccessType ExtendedAccessAttrib AccessLength`,
                     * three bytes (ACPI 6.5 §20.2.5.2; §19.6 `AccessAs` with `AttribBytes` and the
                     * other length-carrying attributes). The OptiPlex 7080 DSDT pairs it with
                     * `Connection` in its GenericSerialBus fields. Later fields fail closed.
                     */
                    let _access_type = context.next()?;
                    let _access_attrib = context.next()?;
                    let _access_length = context.next()?;
                    access_unimplemented = true;
                }
                _ => {
                    context.current_block.pc -= 1;
                    // TODO: this should only ever be a nameseg really...
                    let field_name = context.namestring()?;
                    let field_length = context.pkglength()?;

                    let field = Object::FieldUnit(FieldUnit {
                        kind: kind.clone(),
                        bit_index: field_offset,
                        bit_length: field_length,
                        flags: FieldFlags(flags),
                        access_unimplemented,
                    });
                    self.namespace.lock().insert(field_name.resolve(&context.current_scope)?, field.wrap())?;

                    field_offset += field_length;
                }
            }
        }

        Ok(())
    }

    fn do_binary_maths(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(left), Argument::Object(right), target] = &op.arguments[0..3] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let target2 = if op.op == Opcode::Divide { Some(&op.arguments[3]) } else { None };

        let left = left.clone().unwrap_transparent_reference().as_integer()?;
        let right = right.clone().unwrap_transparent_reference().as_integer()?;
        // LOGANFY(Fix 2): `Divide`/`Mod` by zero panicked in `wrapping_div_euclid`/`wrapping_rem`.
        // It is an AML error (ACPI 6.5 §19.6 `Divide`, `Mod`; ACPICA returns AE_AML_DIVIDE_BY_ZERO).
        if matches!(op.op, Opcode::Divide | Opcode::Mod) && right == 0 {
            return Err(AmlError::MalformedAml("Divide or Mod by zero"));
        }

        let result = match op.op {
            Opcode::Add => left.wrapping_add(right),
            Opcode::Subtract => left.wrapping_sub(right),
            Opcode::Multiply => left.wrapping_mul(right),
            Opcode::Divide => {
                if let Some(remainder) = target2 {
                    self.do_store(remainder, Object::Integer(left.wrapping_rem(right)).wrap())?;
                }
                left.wrapping_div_euclid(right)
            }
            Opcode::ShiftLeft => left.wrapping_shl(right as u32),
            Opcode::ShiftRight => left.wrapping_shr(right as u32),
            Opcode::Mod => left.wrapping_rem(right),
            Opcode::Nand => !(left & right),
            Opcode::And => left & right,
            Opcode::Or => left | right,
            Opcode::Nor => !(left | right),
            Opcode::Xor => left ^ right,
            _ => unreachable!("do_binary_maths is only called for these opcodes"),
        };

        let result = Object::Integer(result).wrap();
        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_unary_maths(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        /*
         * LOGANFY(Fix 1, d): these ops are started with a TermArg *and* a Target (see where
         * `Opcode::Not`/`FindSetLeftBit`/`FindSetRightBit` start their in-flight op), so the stock
         * one-argument pattern panicked on every use. Take both, store the result to the Target
         * and return it (ACPI 6.5 §19.6 `Not`, `FindSetLeftBit`, `FindSetRightBit`; §20.2.5.4
         * Expression Opcodes Encoding: `DefNot := NotOp Operand Target`, and likewise).
         */
        let [Argument::Object(operand), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let operand = operand.clone().unwrap_transparent_reference().as_integer()?;

        let result = match op.op {
            Opcode::FindSetLeftBit => {
                /*
                 * LOGANFY(Fix 1, d): the one-based position of the most significant set bit, so
                 * `FindSetLeftBit(1) == 1`. Stock returned `leading_zeros + 1` (64 for 1). The
                 * integer width follows the DSDT revision.
                 */
                if operand == 0 {
                    0
                } else if self.dsdt_revision >= 2 {
                    (u64::BITS - operand.leading_zeros()) as u64
                } else {
                    (u32::BITS - (operand as u32).leading_zeros()) as u64
                }
            }
            Opcode::FindSetRightBit => {
                if operand == 0 {
                    0
                } else {
                    (operand.trailing_zeros() + 1) as u64
                }
            }
            Opcode::Not => {
                // LOGANFY(Fix 1, d): `Not` is the bitwise one's complement, not a logical not
                // (that is `LNot`), truncated to a revision-1 table's 32-bit integers.
                if self.dsdt_revision >= 2 { !operand } else { !operand & 0xffff_ffff }
            }
            _ => unreachable!("do_unary_maths is only called for these opcodes"),
        };

        let result = self.do_store(target, Object::Integer(result).wrap())?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_logical_op(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        if op.op == Opcode::LNot {
            let [Argument::Object(operand)] = &op.arguments[..] else {
                return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
            };
            let operand = operand.clone().unwrap_transparent_reference().as_integer()?;
            // LOGANFY(Fix 3): Ones at the table's integer width (ACPICA `exoparg1.c` returns
            // ACPI_UINT64_MAX and `dswexec.c` truncates results in a revision-1 table).
            let ones = if self.dsdt_revision >= 2 { u64::MAX } else { u64::from(u32::MAX) };
            let result = if operand == 0 { ones } else { 0 };

            context.contribute_arg(Argument::Object(Object::Integer(result).wrap()));
            context.retire_op(op);
            return Ok(());
        }

        let [Argument::Object(left), Argument::Object(right)] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let left = left.clone().unwrap_transparent_reference();
        let right = right.clone().unwrap_transparent_reference();

        let int_bytes = if self.dsdt_revision >= 2 { 8 } else { 4 };
        /*
         * LOGANFY(Fix 3, 1): stock read the first 4 bytes of a String or Buffer as an Integer and
         * compared those, so a longer Buffer (every 16-byte `_DSM` UUID) panicked, and Strings and
         * Buffers compared numerically instead of byte by byte. ACPICA `AcpiExDoLogicalOp`
         * (`exmisc.c`) converts the right operand to the left's type and compares Strings and
         * Buffers lexicographically: bytes up to the shorter length, then the lengths. That is
         * the slice ordering below (ACPI 6.5 §19.6 `LEqual`, `LGreater`, `LLess`). Integer
         * operands are unchanged.
         */
        let result = match op.op {
            Opcode::LAnd | Opcode::LOr => {
                // LOGANFY(Fix 3, 1, review): both operands are `ARGI_INTEGER` (`acopcode.h:231,249`),
                // converted by `AcpiExConvertToInteger(.., ACPI_IMPLICIT_CONVERSION)`
                // (`exresop.c:398-407`). Fix 3 as merged read a String's first 4 raw
                // bytes, so `LAnd (One, "0")` read 0x30 and was true where stock failed closed;
                // ACPICA reads the String as hex.
                let (left, right) =
                    (implicit_to_integer(&left, int_bytes)?, implicit_to_integer(&right, int_bytes)?);
                if op.op == Opcode::LAnd { left > 0 && right > 0 } else { left > 0 || right > 0 }
            }
            _ => {
                /*
                 * LOGANFY(Fix 3, 1, review): the right operand converts to the left's type in
                 * every arm, as ACPICA `AcpiExDoLogicalOp` does (`exmisc.c:470-487`): an Integer
                 * left takes `AcpiExConvertToInteger(.., ACPI_IMPLICIT_CONVERSION)` and a String
                 * left `AcpiExConvertToString(.., ACPI_IMPLICIT_CONVERT_HEX)`. Stock required the
                 * same type and failed closed otherwise. Any other right type (a Package, say)
                 * stays an error: ACPICA rejects it at operand resolution (`acopcode.h:232`
                 * `ARGI_COMPUTEDATA`; `exresop.c:483-501`, AE_AML_OPERAND_TYPE).
                 */
                let ordering = match *left {
                    Object::Integer(left) => left.cmp(&implicit_to_integer(&right, int_bytes)?),
                    Object::String(ref left) => {
                        left.as_bytes().cmp(implicit_to_hex_string(&right, int_bytes)?.as_bytes())
                    }
                    Object::Buffer(ref left) => {
                        // ACPICA converts a String operand with its NUL (`exconvrt.c`
                        // `AcpiExConvertToBuffer`, length + 1); `to_buffer` does not.
                        let right = match *right {
                            Object::String(ref value) => [value.as_bytes(), &[0]].concat(),
                            _ => right.to_buffer(int_bytes)?,
                        };
                        left.as_slice().cmp(right.as_slice())
                    }
                    _ => Err(AmlError::InvalidOperationOnObject { op: Operation::LogicalOp, typ: left.typ() })?,
                };
                match op.op {
                    Opcode::LNotEqual => ordering.is_ne(),
                    Opcode::LLessEqual => ordering.is_le(),
                    Opcode::LGreaterEqual => ordering.is_ge(),
                    Opcode::LEqual => ordering.is_eq(),
                    Opcode::LGreater => ordering.is_gt(),
                    Opcode::LLess => ordering.is_lt(),
                    _ => unreachable!("do_logical_op is only called for these opcodes"),
                }
            }
        };
        let result = if result { Object::Integer(u64::MAX) } else { Object::Integer(0) };

        context.contribute_arg(Argument::Object(result.wrap()));
        context.retire_op(op);
        Ok(())
    }

    fn do_to_buffer(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(operand), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let operand = operand.clone().unwrap_transparent_reference();

        let result = match *operand {
            Object::Buffer(ref bytes) => Object::Buffer(bytes.clone()),
            Object::Integer(value) => {
                if self.dsdt_revision >= 2 {
                    Object::Buffer(value.to_le_bytes().to_vec())
                } else {
                    Object::Buffer((value as u32).to_le_bytes().to_vec())
                }
            }
            Object::String(ref value) => {
                // XXX: an empty string is converted to an empty buffer, *without* the null-terminator
                if value.is_empty() {
                    Object::Buffer(vec![])
                } else {
                    let mut bytes = value.as_bytes().to_vec();
                    bytes.push(b'\0');
                    check_object_size(bytes.len())?;
                    Object::Buffer(bytes)
                }
            }
            _ => Err(AmlError::InvalidOperationOnObject { op: Operation::ToBuffer, typ: operand.typ() })?,
        }
        .wrap();

        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_to_integer(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(operand), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let operand = operand.clone().unwrap_transparent_reference();

        let result = match *operand {
            Object::Integer(value) => Object::Integer(value),
            Object::Buffer(ref bytes) => {
                /*
                 * The spec says this should respect the revision of the current definition block.
                 * Apparently, the NT interpreter always uses the first 8 bytes of the buffer.
                 */
                let mut to_interpret = [0u8; 8];
                (to_interpret[0..usize::min(bytes.len(), 8)]).copy_from_slice(bytes);
                Object::Integer(u64::from_le_bytes(to_interpret))
            }
            Object::String(ref value) => {
                /*
                 * This is about the same level of effort as ACPICA puts in. The uACPI test suite
                 * has tests that this fails - namely because of support for octal, signs, strings
                 * that won't fit in a `u64` etc. We probably need to write a more robust parser
                 * 'real' parser to handle those cases.
                 */
                if let Some(value) = value.strip_prefix("0x") {
                    let parsed = u64::from_str_radix(value, 16).map_err(|_| {
                        AmlError::InvalidOperationOnObject { op: Operation::ToInteger, typ: ObjectType::String }
                    })?;
                    Object::Integer(parsed)
                } else {
                    let parsed = str::parse::<u64>(value).map_err(|_| AmlError::InvalidOperationOnObject {
                        op: Operation::ToInteger,
                        typ: ObjectType::String,
                    })?;
                    Object::Integer(parsed)
                }
            }
            _ => Err(AmlError::InvalidOperationOnObject { op: Operation::ToBuffer, typ: operand.typ() })?,
        }
        .wrap();

        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_to_string(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(source), Argument::Object(length), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let source = source.clone().unwrap_transparent_reference();
        let source = source.as_buffer()?;
        let length = length.clone().unwrap_transparent_reference().as_integer()? as usize;

        let result = if source.is_empty() {
            Object::String(String::new())
        } else {
            let mut buffer = source.split_inclusive(|b| *b == b'\0').next().unwrap();
            if length < usize::MAX {
                buffer = &buffer[0..usize::min(length, buffer.len())];
            }
            let string = str::from_utf8(buffer).map_err(|_| AmlError::InvalidOperationOnObject {
                op: Operation::ToString,
                typ: ObjectType::Buffer,
            })?;
            Object::String(string.to_string())
        }
        .wrap();

        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    /// Perform a `ToDecimalString` or `ToHexString` operation
    fn do_to_dec_hex_string(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(operand), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let operand = operand.clone().unwrap_transparent_reference();

        let result = match *operand {
            Object::String(ref value) => Object::String(value.clone()),
            Object::Integer(value) => match op.op {
                Opcode::ToDecimalString => Object::String(value.to_string()),
                Opcode::ToHexString => Object::String(alloc::format!("{value:#x}")),
                _ => unreachable!("do_to_dec_hex_string is only called for these opcodes"),
            },
            Object::Buffer(ref bytes) => {
                if bytes.is_empty() {
                    Object::String(String::new())
                } else {
                    let mut string = String::new();
                    for byte in bytes {
                        let as_str = match op.op {
                            Opcode::ToDecimalString => alloc::format!("{byte},"),
                            Opcode::ToHexString => alloc::format!("{byte:#04X},"),
                            _ => unreachable!("do_to_dec_hex_string is only called for these opcodes"),
                        };
                        string.push_str(&as_str);
                    }
                    // Remove last comma, if present
                    if !string.is_empty() {
                        string.pop();
                    }
                    check_object_size(string.len())?;
                    Object::String(string)
                }
            }
            _ => Err(AmlError::InvalidOperationOnObject { op: Operation::ToDecOrHexString, typ: operand.typ() })?,
        }
        .wrap();

        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_mid(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(source), Argument::Object(index), Argument::Object(length), target] =
            &op.arguments[..]
        else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let index = index.clone().unwrap_transparent_reference().as_integer()? as usize;
        let length = length.clone().unwrap_transparent_reference().as_integer()? as usize;
        // LOGANFY(Fix 1, G1): a Local/Arg operand is a transparent reference; read its value.
        let source = source.clone().unwrap_transparent_reference();

        /*
         * LOGANFY(Fix 2): stock bounded the end by `index + len`, which runs past the end for any
         * `index > 0`, and added unchecked. The result is at most `Length` bytes from `Index`,
         * clipped to the source (ACPI 6.5 §19.6 `Mid`).
         */
        let result = match *source {
            Object::String(ref string) => {
                if index >= string.len() {
                    Object::String(String::new())
                } else {
                    let upper = usize::min(index.saturating_add(length), string.len());
                    let chars = string
                        .get(index..upper)
                        .ok_or(AmlError::MalformedAml("Mid inside a non-ASCII character"))?;
                    Object::String(String::from(chars))
                }
            }
            Object::Buffer(ref buffer) => {
                if index >= buffer.len() {
                    Object::Buffer(vec![])
                } else {
                    let upper = usize::min(index.saturating_add(length), buffer.len());
                    let bytes = &buffer[index..upper];
                    Object::Buffer(bytes.to_vec())
                }
            }
            _ => Err(AmlError::InvalidOperationOnObject { op: Operation::Mid, typ: source.typ() })?,
        }
        .wrap();

        self.do_store(target, result.clone())?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_concat(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(source1), Argument::Object(source2), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let source1 = source1.clone().unwrap_transparent_reference();
        let source2 = source2.clone().unwrap_transparent_reference();

        fn resolve_as_string(obj: &Object) -> String {
            match obj {
                Object::Uninitialized => "[Uninitialized Object]".to_string(),
                Object::Buffer(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                Object::BufferField { .. } => "[Buffer Field]".to_string(),
                Object::Device => "[Device]".to_string(),
                Object::Event(_) => "[Event]".to_string(),
                Object::FieldUnit(_) => "[Field]".to_string(),
                Object::Integer(value) => value.to_string(),
                Object::Method { .. } | Object::NativeMethod { .. } => "[Control Method]".to_string(),
                Object::Mutex { .. } => "[Mutex]".to_string(),
                Object::Reference { inner, .. } => resolve_as_string(&(inner.clone().unwrap_reference())),
                Object::OpRegion(_) => "[Operation Region]".to_string(),
                Object::Package(_) => "[Package]".to_string(),
                Object::PowerResource { .. } => "[Power Resource]".to_string(),
                Object::Processor { .. } => "[Processor]".to_string(),
                // TODO: what even is one of these??
                // LOGANFY(Fix 2): stock `todo!()`; the interpreter never builds a RawDataBuffer.
                Object::RawDataBuffer => unreachable!("no RawDataBuffer is ever constructed"),
                Object::String(value) => value.clone(),
                Object::ThermalZone => "[Thermal Zone]".to_string(),
                Object::Debug => "[Debug Object]".to_string(),
            }
        }

        let result = match source1.typ() {
            ObjectType::Integer => {
                let source1 = source1.as_integer()?;
                let source2 = source2.to_integer(if self.dsdt_revision >= 2 { 8 } else { 4 })?;
                let mut buffer = Vec::new();
                if self.dsdt_revision >= 2 {
                    buffer.extend_from_slice(&source1.to_le_bytes());
                    buffer.extend_from_slice(&source2.to_le_bytes());
                } else {
                    buffer.extend_from_slice(&(source1 as u32).to_le_bytes());
                    buffer.extend_from_slice(&(source2 as u32).to_le_bytes());
                }
                Object::Buffer(buffer).wrap()
            }
            ObjectType::Buffer => {
                let source1 = source1.as_buffer()?;
                let source2 = source2.to_buffer(if self.dsdt_revision >= 2 { 8 } else { 4 })?;
                check_object_size(source1.len() + source2.len())?;
                let mut buffer = source1.to_vec();
                buffer.extend(source2);
                Object::Buffer(buffer).wrap()
            }
            _ => {
                // `from_utf8_lossy` can make a Buffer's text up to three times its length.
                let source1 = resolve_as_string(&source1);
                let source2 = resolve_as_string(&source2);
                check_object_size(source1.len() + source2.len())?;
                Object::String(source1 + &source2).wrap()
            }
        };

        let result = self.do_store(target, result)?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_from_bcd(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        // LOGANFY(Fix 1, d): the same arity defect as `do_unary_maths`: `FromBCD` is started with
        // a TermArg and a Target (ACPI 6.5 §19.6 `FromBCD`).
        let [Argument::Object(value), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let mut value = value.clone().unwrap_transparent_reference().as_integer()?;

        let mut result = 0;
        let mut i = 1;
        while value > 0 {
            result += (value & 0x0f) * i;
            i *= 10;
            value >>= 4;
        }

        let result = self.do_store(target, Object::Integer(result).wrap())?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_to_bcd(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        // LOGANFY(Fix 1, d): as for `FromBCD` (ACPI 6.5 §19.6 `ToBCD`).
        let [Argument::Object(value), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let mut value = value.clone().unwrap_transparent_reference().as_integer()?;

        let mut result = 0;
        let mut i = 0;
        while value > 0 {
            result |= (value % 10) << (4 * i);
            value /= 10;
            i += 1;
        }

        let result = self.do_store(target, Object::Integer(result).wrap())?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_size_of(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(object)] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let object = object.clone().unwrap_transparent_reference();

        let result = match *object {
            Object::Buffer(ref buffer) => buffer.len(),
            Object::String(ref str) => str.len(),
            Object::Package(ref package) => package.len(),
            _ => Err(AmlError::InvalidOperationOnObject { op: Operation::SizeOf, typ: object.typ() })?,
        };

        context.contribute_arg(Argument::Object(Object::Integer(result as u64).wrap()));
        context.retire_op(op);
        Ok(())
    }

    fn do_index(&self, context: &mut MethodContext, op: OpInFlight) -> Result<(), AmlError> {
        let [Argument::Object(object), Argument::Object(index_value), target] = &op.arguments[..] else {
            return Err(AmlError::MalformedAml("operator arguments of an unexpected kind"));
        };
        let object = object.clone().unwrap_transparent_reference();
        let index_value = index_value.clone().unwrap_transparent_reference().as_integer()?;

        let result = match *object {
            Object::Buffer(ref buffer) => {
                if index_value as usize >= buffer.len() {
                    Err(AmlError::IndexOutOfBounds)?
                }

                // LOGANFY(Fix 1, d): an `Index` reference, so a store through it writes the byte.
                Object::Reference {
                    kind: ReferenceKind::Index,
                    inner: Object::BufferField {
                        buffer: object.clone(),
                        offset: index_value as usize * 8,
                        length: 8,
                    }
                    .wrap(),
                }
            }
            Object::String(ref string) => {
                if index_value as usize >= string.len() {
                    Err(AmlError::IndexOutOfBounds)?
                }

                // LOGANFY(Fix 1, d): an `Index` reference, so a store through it writes the byte.
                Object::Reference {
                    kind: ReferenceKind::Index,
                    inner: Object::BufferField {
                        buffer: object.clone(),
                        offset: index_value as usize * 8,
                        length: 8,
                    }
                    .wrap(),
                }
            }
            Object::Package(ref package) => {
                let Some(element) = package.get(index_value as usize) else { Err(AmlError::IndexOutOfBounds)? };
                // LOGANFY(Fix 1, d): an `Index` reference, so a store through it replaces the element.
                Object::Reference { kind: ReferenceKind::Index, inner: element.clone() }
            }
            _ => Err(AmlError::IndexOutOfBounds)?,
        }
        .wrap();

        self.do_store(target, result.clone())?;
        context.contribute_arg(Argument::Object(result));
        context.retire_op(op);
        Ok(())
    }

    fn do_store(&self, target: &Argument, object: WrappedObject) -> Result<WrappedObject, AmlError> {
        let object = object.unwrap_transparent_reference();

        /*
         * LOGANFY(Fix 1, d): a store through an object reference (stock: `todo!()`). Handled before
         * the object token is taken, because storing to a `RefOf` target is a store to the
         * referenced object and recurses.
         *   - `RefOf`: store to the referenced named object, with the same implicit conversion as
         *     a store to that name (ACPI 6.5 §19.6 `Store`, §19.6 `RefOf`; §19.3.5 ASL Data Types).
         *   - `Index` of a Package: the element is replaced by a copy of the source, with no
         *     conversion (ACPI 6.5 §19.6 `Index`).
         *   - `Index` of a Buffer/String (a one-byte `BufferField`): the source's low byte is
         *     written, an Integer's least-significant byte or a Buffer's/String's first byte.
         */
        if let Argument::Object(target) = target
            && let Object::Reference { kind: kind @ (ReferenceKind::RefOf | ReferenceKind::Index), ref inner } =
                **target
        {
            let to_return = object.clone();
            match (kind, &**inner) {
                (ReferenceKind::Index, Object::BufferField { .. }) => {
                    let byte = match *object {
                        Object::Integer(value) => value as u8,
                        Object::Buffer(ref bytes) => *bytes.first().ok_or(AmlError::IndexOutOfBounds)?,
                        Object::String(ref string) => {
                            *string.as_bytes().first().ok_or(AmlError::IndexOutOfBounds)?
                        }
                        _ => Err(AmlError::ObjectNotOfExpectedType {
                            expected: ObjectType::Integer,
                            got: object.typ(),
                        })?,
                    };
                    let token = self.object_token.lock();
                    unsafe { inner.gain_mut(&token) }.write_buffer_field(&[byte], &token)?;
                }
                (ReferenceKind::Index, _) => {
                    check_acyclic(inner, &object)?;
                    let token = self.object_token.lock();
                    unsafe {
                        *inner.gain_mut(&token) = object.gain_mut(&token).clone();
                    }
                }
                _ => {
                    self.do_store(&Argument::Object(inner.clone()), object)?;
                }
            }
            return Ok(to_return);
        }

        // LOGANFY(Fix 3, 8): a store of an object into itself changes nothing (ACPICA
        // `exstorob.c` returns early too), and must not take two mutable views of one object.
        if let Argument::Object(target) = target
            && target.ptr_eq(&object)
        {
            return Ok(object);
        }

        let token = self.object_token.lock();

        /*
         * TODO: stores should do more implicit conversion to the type of the destination in some
         * cases, in line with section 19.3.5 of the spec. It's not clear what existing
         * interpreters do - NT may just be memcpying objects over each other...
         *
         * TODO: stores to fields with `BufferAcc` can actually return a value of the store that
         * differs from what was written into the field. This is used for complex field types with
         * a write-then-read pattern. The return value is then used as the 'result' of the storing
         * expression.
         */
        let to_return = object.clone();

        match target {
            Argument::Null => {}
            Argument::Object(target) => match unsafe { target.gain_mut(&token) } {
                Object::Integer(target) => match unsafe { object.gain_mut(&token) } {
                    Object::Integer(value) => {
                        *target = *value;
                    }
                    Object::BufferField { .. } => {
                        let mut buffer = [0u8; 8];
                        unsafe { object.gain_mut(&token) }.read_buffer_field(&mut buffer)?;
                        let value = u64::from_le_bytes(buffer);
                        *target = value;
                    }
                    Object::FieldUnit(field) => {
                        // TODO: not sure if we should convert buffers to integers if needed here?
                        *target = self.do_field_read(field)?.as_integer()?;
                    }
                    _ => {
                        let as_integer = object.to_integer(if self.dsdt_revision >= 2 { 8 } else { 4 })?;
                        *target = as_integer;
                    }
                },
                Object::BufferField { .. } => match unsafe { object.gain_mut(&token) } {
                    Object::Integer(value) => {
                        unsafe { target.gain_mut(&token) }.write_buffer_field(&value.to_le_bytes(), &token)?;
                    }
                    Object::Buffer(value) => {
                        unsafe { target.gain_mut(&token) }.write_buffer_field(value.as_slice(), &token)?;
                    }
                    // LOGANFY(Fix 2): stock `panic!()` on any other source (a String, a Package...);
                    // the implicit conversion is OT-86 L4.
                    _ => return Err(fail_closed("Store of this object type to a buffer field")),
                },
                Object::FieldUnit(field) => self.do_field_write(field, object)?,
                Object::Reference { kind, inner } => {
                    match kind {
                        // LOGANFY(Fix 1, d): stores through `RefOf`/`Index` are handled above.
                        ReferenceKind::RefOf | ReferenceKind::Index => unreachable!(),
                        ReferenceKind::LocalOrArg => {
                            if let Object::Reference { kind: _inner_kind, inner: inner_inner } = &**inner {
                                check_acyclic(inner_inner, &object)?;
                                // TODO: this should store into the reference, potentially doing an
                                // implicit cast
                                unsafe {
                                    *inner_inner.gain_mut(&token) = object.gain_mut(&token).clone();
                                }
                            } else {
                                check_acyclic(inner, &object)?;
                                // Overwrite the value
                                unsafe {
                                    *inner.gain_mut(&token) = object.gain_mut(&token).clone();
                                }
                            }
                        }
                        // LOGANFY(Fix 2): a name `CondRefOf` found missing is not a store target
                        // (stock: `todo!()`).
                        ReferenceKind::Unresolved => {
                            return Err(AmlError::MalformedAml("store to a name that does not exist"));
                        }
                    }
                }
                Object::Debug => {
                    self.handler.handle_debug(&object);
                }
                /*
                 * LOGANFY(Fix 3, 8): Store to a named Buffer, as ACPICA `exstorob.c`
                 * `AcpiExStoreBufferToBuffer` after `AcpiExConvertToTargetType`: the source is
                 * converted to a Buffer (an Integer as its integer-width bytes, a String with its
                 * NUL: `exconvrt.c` `AcpiExConvertToBuffer`), then copied in. The target keeps its
                 * size, zero-filled or truncated (ACPI 6.5 §19.3.5), except that a zero-length
                 * target takes the source's length (`STORE_GROWS_EMPTY_BUFFER`). Stock `panic!`.
                 */
                Object::Buffer(target) => {
                    let source: Vec<u8> = match *object {
                        Object::Buffer(ref bytes) => bytes.clone(),
                        Object::Integer(value) => {
                            value.to_le_bytes()[..if self.dsdt_revision >= 2 { 8 } else { 4 }].to_vec()
                        }
                        Object::String(ref value) => [value.as_bytes(), &[0]].concat(),
                        _ => return Err(fail_closed("Store of this object type to a Buffer")),
                    };
                    if target.is_empty() && STORE_GROWS_EMPTY_BUFFER {
                        check_object_size(source.len())?;
                        *target = source;
                    } else {
                        let n = usize::min(target.len(), source.len());
                        target[..n].copy_from_slice(&source[..n]);
                        target[n..].fill(0);
                    }
                }
                /*
                 * LOGANFY(Fix 3, 8): Store of a String to a named String takes the source, at its
                 * length (ACPICA `exstorob.c` `AcpiExStoreStringToString`). Converting another type
                 * to a String on Store (`AcpiExConvertToString`, hex formatting) is not built and
                 * fails closed. Stock `panic!`.
                 */
                Object::String(target) => match *object {
                    Object::String(ref value) => *target = value.clone(),
                    _ => return Err(fail_closed("Store of this object type to a String")),
                },
                // LOGANFY(Fix 2): stock `panic!`. Store to a named Package (and the other types)
                // fails closed.
                _ => return Err(fail_closed("Store to a named object of this type")),
            },

            // LOGANFY(Fix 2): stock `todo!()`/`panic!()`; a target is always an object or null.
            Argument::Namestring(_) => return Err(AmlError::MalformedAml("store to a non-object target")),
            Argument::ByteData(_) | Argument::DWordData(_) | Argument::TrackedPc(_) | Argument::PkgLength(_) => {
                return Err(AmlError::MalformedAml("store to a non-object target"));
            }
        }

        Ok(to_return)
    }

    /// LOGANFY(Fix 1, G2/G3): the value of a named or referenced object, as a TermArg evaluates it.
    /// A field unit is read from its region (as the stock TermArg path already did), and a buffer
    /// field is read from its buffer: an Integer when it fits the definition block's integer width
    /// (32 bits for a revision-1 DSDT, else 64), otherwise a Buffer (ACPI 6.5 §19.6 `CreateField`,
    /// §19.3.5 ASL Data Types). Every other object is returned as it is.
    fn read_named_value(&self, object: WrappedObject) -> Result<WrappedObject, AmlError> {
        match *object {
            Object::FieldUnit(ref field) => self.do_field_read(field),
            Object::BufferField { length, .. } => {
                let integer_bits = if self.dsdt_revision >= 2 { 64 } else { 32 };
                if length <= integer_bits {
                    let mut bytes = [0u8; 8];
                    object.read_buffer_field(&mut bytes)?;
                    Ok(Object::Integer(u64::from_le_bytes(bytes)).wrap())
                } else {
                    let mut bytes = vec![0u8; length.div_ceil(8)];
                    object.read_buffer_field(&mut bytes)?;
                    Ok(Object::Buffer(bytes).wrap())
                }
            }
            _ => Ok(object),
        }
    }

    /// Do a read from a field by performing one or more well-formed accesses to the underlying
    /// operation regions, and then shifting and masking the resulting value as appropriate. Will
    /// return either an `Integer` or `Buffer` as appropriate, guided by the size of the field
    /// and expected integer size (as per the DSDT revision).
    fn do_field_read(&self, field: &FieldUnit) -> Result<WrappedObject, AmlError> {
        // LOGANFY(Fix 1, c): see `FieldUnit::access_unimplemented`.
        if field.access_unimplemented {
            return Err(fail_closed("read of a field after Connection/extended AccessAs"));
        }
        let needs_buffer = if self.dsdt_revision >= 2 { field.bit_length > 64 } else { field.bit_length > 32 };
        let access_width_bits = field.flags.access_type_bytes()? * 8;

        trace!("AML field read. Field = {:?}", field);

        // TODO: if the field needs to be locked, acquire/release a global mutex?

        enum Output {
            Integer([u8; 8]),
            Buffer(Vec<u8>),
        }
        let mut output = if needs_buffer {
            /*
             * LOGANFY(Fix 2): a field's length is a PkgLength (up to 2^28 bits), so cap the
             * allocation.
             * LOGANFY(Fix 3, 9): stock sized the Buffer in bytes as `bit_length` rounded up to 8,
             * eight times the field. ACPICA returns `ACPI_ROUND_BITS_UP_TO_BYTES (BitLength)`
             * bytes (`exfield.c` `AcpiExReadDataFromField`).
             */
            let byte_length = field.bit_length.div_ceil(8);
            if byte_length > MAX_OBJECT_BYTES {
                return Err(AmlError::MalformedAml("field longer than the object size limit"));
            }
            Output::Buffer(vec![0; byte_length])
        } else {
            Output::Integer([0; 8])
        };
        let output_bytes = match &mut output {
            Output::Buffer(bytes) => bytes.as_mut_slice(),
            Output::Integer(value) => value,
        };

        /*
         * TODO: it might be worth having a fast path here for reads that don't do weird
         * unaligned accesses, which I'm guessing might be relatively common on real
         * hardware? Eg. single native read + mask
         */

        /*
         * Break the field read into native reads that respect the region's access width.
         * Copy each potentially-unaligned part into the destination's bit range.
         */
        let native_accesses_needed = (field.bit_length + (field.bit_index % access_width_bits))
            .next_multiple_of(access_width_bits)
            / access_width_bits;
        let mut read_so_far = 0;
        for i in 0..native_accesses_needed {
            let aligned_offset = object::align_down(field.bit_index + i * access_width_bits, access_width_bits);
            let raw = self.field_datum_read(&field.kind, aligned_offset / 8, access_width_bits / 8)?;
            let src_index = if i == 0 { field.bit_index % access_width_bits } else { 0 };
            let remaining_length = field.bit_length - read_so_far;
            let length = if i == 0 {
                usize::min(remaining_length, access_width_bits - (field.bit_index % access_width_bits))
            } else {
                usize::min(remaining_length, access_width_bits)
            };

            object::copy_bits(&raw.to_le_bytes(), src_index, output_bytes, read_so_far, length);
            read_so_far += length;
        }

        match output {
            Output::Buffer(bytes) => Ok(Object::Buffer(bytes).wrap()),
            Output::Integer(value) => Ok(Object::Integer(u64::from_le_bytes(value)).wrap()),
        }
    }

    fn do_field_write(&self, field: &FieldUnit, value: WrappedObject) -> Result<(), AmlError> {
        trace!("AML field write. Field = {:?}. Value = {}", field, value);
        // LOGANFY(Fix 1, c): see `FieldUnit::access_unimplemented`.
        if field.access_unimplemented {
            return Err(fail_closed("write of a field after Connection/extended AccessAs"));
        }

        let value_bytes = match &*value {
            Object::Integer(value) => &value.to_le_bytes() as &[u8],
            Object::Buffer(bytes) => bytes,
            _ => Err(AmlError::ObjectNotOfExpectedType { expected: ObjectType::Integer, got: value.typ() })?,
        };
        let access_width_bits = field.flags.access_type_bytes()? * 8;

        // TODO: if the region wants locking, do that

        // TODO: maybe also a fast path for writes

        let native_accesses_needed = (field.bit_length + (field.bit_index % access_width_bits))
            .next_multiple_of(access_width_bits)
            / access_width_bits;
        let mut written_so_far = 0;

        for i in 0..native_accesses_needed {
            let aligned_offset = object::align_down(field.bit_index + i * access_width_bits, access_width_bits);
            let dst_index = if i == 0 { field.bit_index % access_width_bits } else { 0 };

            /*
             * If we're not going to write a whole native access, respect the field's
             * update rule. If we're meant to preserve the surrounding bits, we need to do
             * a read first.
             */
            let mut bytes = if dst_index > 0 || (field.bit_length - written_so_far) < access_width_bits {
                match field.flags.update_rule()? {
                    FieldUpdateRule::Preserve => self
                        .field_datum_read(&field.kind, aligned_offset / 8, access_width_bits / 8)?
                        .to_le_bytes(),
                    FieldUpdateRule::WriteAsOnes => [0xff; 8],
                    FieldUpdateRule::WriteAsZeros => [0; 8],
                }
            } else {
                [0; 8]
            };

            let remaining_length = field.bit_length - written_so_far;
            let length = if i == 0 {
                usize::min(remaining_length, access_width_bits - (field.bit_index % access_width_bits))
            } else {
                usize::min(remaining_length, access_width_bits)
            };

            object::copy_bits(value_bytes, written_so_far, &mut bytes, dst_index, length);
            self.field_datum_write(
                &field.kind,
                aligned_offset / 8,
                access_width_bits / 8,
                u64::from_le_bytes(bytes),
            )?;
            written_so_far += length;
        }

        Ok(())
    }

    /*
     * LOGANFY(Fix 3, 5 and 6): one datum of a field, `width` bytes at byte `offset` of the field's
     * storage. Stock handled only a plain region field; BankField and IndexField were `todo!()`
     * (Fix 1/2: fail closed). As ACPICA `exfldio.c` `AcpiExFieldDatumIo`:
     *   - BankField: write the bank value to the bank register (itself a field), then access the
     *     region as for a plain field;
     *   - IndexField: write the datum's byte offset to the index register, then read or write
     *     the data register. The offset already includes the field's own start, rounded down to
     *     the access width (ACPICA `exprep.c` sets `IndexField.Value` to that).
     * Either fails if the value does not fit its register (ACPICA `AcpiExRegisterOverflow`,
     * AE_AML_REGISTER_LIMIT).
     */
    fn field_datum_read(&self, kind: &FieldUnitKind, offset: usize, width: usize) -> Result<u64, AmlError> {
        match kind {
            FieldUnitKind::Normal { region } => {
                self.do_native_region_read(Self::op_region(region)?, offset, width)
            }
            FieldUnitKind::Bank { region, bank, bank_value } => {
                self.write_register(bank, *bank_value, "BankField value beyond its bank register")?;
                self.do_native_region_read(Self::op_region(region)?, offset, width)
            }
            FieldUnitKind::Index { index, data } => {
                self.write_register(index, offset as u64, "IndexField offset beyond its index register")?;
                let value = match *self.do_field_read(Self::field_unit(data)?)? {
                    Object::Integer(value) => value,
                    Object::Buffer(ref bytes) => {
                        let mut le = [0u8; 8];
                        let n = usize::min(8, bytes.len());
                        le[..n].copy_from_slice(&bytes[..n]);
                        u64::from_le_bytes(le)
                    }
                    _ => return Err(AmlError::MalformedAml("operator arguments of an unexpected kind")),
                };
                Ok(if width >= 8 { value } else { value & ((1u64 << (width * 8)) - 1) })
            }
        }
    }

    fn field_datum_write(
        &self,
        kind: &FieldUnitKind,
        offset: usize,
        width: usize,
        value: u64,
    ) -> Result<(), AmlError> {
        match kind {
            FieldUnitKind::Normal { region } => {
                self.do_native_region_write(Self::op_region(region)?, offset, width, value)
            }
            FieldUnitKind::Bank { region, bank, bank_value } => {
                self.write_register(bank, *bank_value, "BankField value beyond its bank register")?;
                self.do_native_region_write(Self::op_region(region)?, offset, width, value)
            }
            FieldUnitKind::Index { index, data } => {
                self.write_register(index, offset as u64, "IndexField offset beyond its index register")?;
                self.do_field_write(Self::field_unit(data)?, Object::Integer(value).wrap())
            }
        }
    }

    /// Write `value` to the bank or index register `register`, a field unit.
    fn write_register(
        &self,
        register: &WrappedObject,
        value: u64,
        too_wide: &'static str,
    ) -> Result<(), AmlError> {
        let register = Self::field_unit(register)?;
        if register.bit_length < 64 && value >> register.bit_length != 0 {
            return Err(AmlError::MalformedAml(too_wide));
        }
        self.do_field_write(register, Object::Integer(value).wrap())
    }

    fn op_region(object: &WrappedObject) -> Result<&OpRegion, AmlError> {
        match **object {
            Object::OpRegion(ref region) => Ok(region),
            _ => Err(AmlError::MalformedAml("operator arguments of an unexpected kind")),
        }
    }

    fn field_unit(object: &WrappedObject) -> Result<&FieldUnit, AmlError> {
        match **object {
            Object::FieldUnit(ref field) => Ok(field),
            _ => Err(AmlError::MalformedAml("operator arguments of an unexpected kind")),
        }
    }

    /// Performs an actual read from an operation region. `offset` and `length` must respect the
    /// access requirements of the field being read, and are supplied in **bytes**. This may call
    /// AML methods if required, and may invoke user-supplied handlers.
    fn do_native_region_read(&self, region: &OpRegion, offset: usize, length: usize) -> Result<u64, AmlError> {
        trace!("Native field read. Region = {:?}, offset = {:#x}, length={:#x}", region, offset, length);

        match region.space {
            RegionSpace::SystemMemory => Ok({
                // LOGANFY(Fix 2): checked; stock could overflow the address.
                let address = (region.base as usize)
                    .checked_add(offset)
                    .ok_or(AmlError::MalformedAml("SystemMemory access beyond the address space"))?;
                match length {
                    1 => self.handler.read_u8(address) as u64,
                    2 => self.handler.read_u16(address) as u64,
                    4 => self.handler.read_u32(address) as u64,
                    8 => self.handler.read_u64(address),
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
            }),
            RegionSpace::SystemIO => Ok({
                // LOGANFY(Fix 2): stock added in u16 (an overflow panic, or a wrapped port in release).
                let address = u16::try_from(region.base)
                    .ok()
                    .and_then(|base| base.checked_add(u16::try_from(offset).ok()?))
                    .ok_or(AmlError::MalformedAml("SystemIO access beyond port 0xFFFF"))?;
                match length {
                    1 => self.handler.read_io_u8(address) as u64,
                    2 => self.handler.read_io_u16(address) as u64,
                    4 => self.handler.read_io_u32(address) as u64,
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
            }),
            RegionSpace::PciConfig => {
                let address = self.pci_address_for_device(&region.parent_device_path)?;
                /*
                 * LOGANFY(Fix 1, a): a PCI_Config region's RegionOffset is its base within the
                 * function's configuration space, so the field's byte offset is added to it, as
                 * SystemMemory and SystemIO do above (ACPI 6.5 §19.6 `OperationRegion`). Stock
                 * passed the field offset alone.
                 */
                let offset = (region.base as usize)
                    .checked_add(offset)
                    .filter(|o| *o <= u16::MAX as usize)
                    // LOGANFY(Fix 2): stock truncated the offset to u16 silently.
                    .ok_or(AmlError::MalformedAml("PCI_Config access beyond offset 0xFFFF"))?;
                match length {
                    1 => Ok(self.handler.read_pci_u8(address, offset as u16) as u64),
                    2 => Ok(self.handler.read_pci_u16(address, offset as u16) as u64),
                    4 => Ok(self.handler.read_pci_u32(address, offset as u16) as u64),
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
            }

            RegionSpace::EmbeddedControl
            | RegionSpace::SmBus
            | RegionSpace::SystemCmos
            | RegionSpace::PciBarTarget
            | RegionSpace::Ipmi
            | RegionSpace::GeneralPurposeIo
            | RegionSpace::GenericSerialBus
            | RegionSpace::Pcc
            | RegionSpace::Oem(_) => {
                if let Some(_handler) = self.region_handlers.lock().get(&region.space) {
                    warn!("Custom region handlers aren't actually supported yet.");
                    Err(AmlError::LibUnimplemented)
                } else {
                    Err(AmlError::NoHandlerForRegionAccess(region.space))
                }
            }
        }
    }

    /// Performs an actual write to an operation region. `offset` and `length` must respect the
    /// access requirements of the field being read, and are supplied in **bytes**. This may call
    /// AML methods if required, and may invoke user-supplied handlers.
    fn do_native_region_write(
        &self,
        region: &OpRegion,
        offset: usize,
        length: usize,
        value: u64,
    ) -> Result<(), AmlError> {
        trace!(
            "Native field write. Region = {:?}, offset = {:#x}, length={:#x}, value={:#x}",
            region, offset, length, value
        );

        match region.space {
            RegionSpace::SystemMemory => {
                // LOGANFY(Fix 2): checked; stock could overflow the address.
                let address = (region.base as usize)
                    .checked_add(offset)
                    .ok_or(AmlError::MalformedAml("SystemMemory access beyond the address space"))?;
                match length {
                    1 => self.handler.write_u8(address, value as u8),
                    2 => self.handler.write_u16(address, value as u16),
                    4 => self.handler.write_u32(address, value as u32),
                    8 => self.handler.write_u64(address, value),
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
                Ok(())
            }
            RegionSpace::SystemIO => {
                // LOGANFY(Fix 2): stock added in u16 (an overflow panic, or a wrapped port in release).
                let address = u16::try_from(region.base)
                    .ok()
                    .and_then(|base| base.checked_add(u16::try_from(offset).ok()?))
                    .ok_or(AmlError::MalformedAml("SystemIO access beyond port 0xFFFF"))?;
                match length {
                    1 => self.handler.write_io_u8(address, value as u8),
                    2 => self.handler.write_io_u16(address, value as u16),
                    4 => self.handler.write_io_u32(address, value as u32),
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
                Ok(())
            }
            RegionSpace::PciConfig => {
                let address = self.pci_address_for_device(&region.parent_device_path)?;
                // LOGANFY(Fix 1, a): add the region base, as for the read above.
                let offset = (region.base as usize)
                    .checked_add(offset)
                    .filter(|o| *o <= u16::MAX as usize)
                    // LOGANFY(Fix 2): stock truncated the offset to u16 silently.
                    .ok_or(AmlError::MalformedAml("PCI_Config access beyond offset 0xFFFF"))?;
                match length {
                    1 => self.handler.write_pci_u8(address, offset as u16, value as u8),
                    2 => self.handler.write_pci_u16(address, offset as u16, value as u16),
                    4 => self.handler.write_pci_u32(address, offset as u16, value as u32),
                    // LOGANFY(Fix 2): stock `panic!()`; e.g. QWordAcc on SystemIO or PCI_Config.
                    _ => return Err(AmlError::MalformedAml("an access width this region space cannot do")),
                }
                Ok(())
            }

            RegionSpace::EmbeddedControl
            | RegionSpace::SmBus
            | RegionSpace::SystemCmos
            | RegionSpace::PciBarTarget
            | RegionSpace::Ipmi
            | RegionSpace::GeneralPurposeIo
            | RegionSpace::GenericSerialBus
            | RegionSpace::Pcc
            | RegionSpace::Oem(_) => {
                if let Some(_handler) = self.region_handlers.lock().get(&region.space) {
                    warn!("Custom region handlers aren't actually supported yet.");
                    Err(AmlError::LibUnimplemented)
                } else {
                    Err(AmlError::NoHandlerForRegionAccess(region.space))
                }
            }
        }
    }

    fn pci_address_for_device(&self, path: &AmlName) -> Result<PciAddress, AmlError> {
        /*
         * TODO: it's not ideal to do these reads for every native access. See if we can
         * cache them somewhere?
         */
        let seg = match self.evaluate_if_present(AmlName::from_str("_SEG").unwrap().resolve(path)?, vec![])? {
            Some(value) => value.as_integer()?,
            None => 0,
        };
        let bus = match self.evaluate_if_present(AmlName::from_str("_BBR").unwrap().resolve(path)?, vec![])? {
            Some(value) => value.as_integer()?,
            None => 0,
        };
        let (device, function) = {
            let adr = self.evaluate_if_present(AmlName::from_str("_ADR").unwrap().resolve(path)?, vec![])?;
            let adr = match adr {
                Some(adr) => adr.as_integer()?,
                None => 0,
            };
            (adr.get_bits(16..32), adr.get_bits(0..16))
        };
        Ok(PciAddress::new(seg as u16, bus as u8, device as u8, function as u8))
    }
}

/// A `MethodContext` represents a piece of running AML code - either a real method, or the
/// top-level of an AML table.
///
/// ### Safety
/// `MethodContext` does not keep the lifetime of the underlying AML stream, which for tables is
/// borrowed from the underlying physical mapping. This is because the interpreter needs to
/// preempt method contexts that execute other methods, and these contexts may have disparate
/// lifetimes. This is made safe in the case of methods by the context holding a reference to the
/// method object, but must be handled manually for AML tables.
struct MethodContext {
    current_block: Block,
    block_stack: Vec<Block>,
    in_flight: Vec<OpInFlight>,
    args: [WrappedObject; 8],
    locals: [WrappedObject; 8],
    current_scope: AmlName,

    _method: Option<WrappedObject>,
}

struct Block {
    stream: *const [u8],
    pc: usize,
    kind: BlockKind,
}

impl Block {
    fn stream(&self) -> &[u8] {
        unsafe { &*self.stream }
    }
}

#[derive(PartialEq, Debug)]
pub enum BlockKind {
    Table,
    Method {
        method_scope: AmlName,
    },
    Scope {
        old_scope: AmlName,
    },
    Package,
    VarPackage,
    /// Used for executing the then-branch of an `DefIfElse`. After finishing, it will check for
    /// and skip over an else-branch, if present.
    IfThenBranch,
    While {
        start_pc: usize,
    },
}

/// A `ResolveBehaviour` describes how a name at the top-level should be resolved as part of an
/// operation.
#[derive(Clone, Copy, PartialEq, Debug)]
enum ResolveBehaviour {
    /// Attempt to resolve the name to an object that has already been defined. There are generally
    /// no forward definitions in AML, so this is the usual resolution behaviour for most operands.
    TermArg,
    /// Resolve a name to reference an object. This is used when an operation needs to operate on
    /// the object itself, rather than evaluate it to a value. For example, accessing a `FieldUnit` would
    /// read a value from the field with `TermArg`, but resolves to the `FieldUnit` with
    /// this behaviour.
    SuperName,
    /// Behaves the same as `SuperName` if the object exists, but resolves successfully to an
    /// unresolved reference if the object does not exist. Used by `DefCondRefOf`.
    SuperNameIfExists,
    /// `SuperName`, but can also be `NullName`
    Target,
    /// Surrogate argument, used by `DefPackage` and `DefVarPackage`. Only one of these is emitted,
    /// but represents parsing of potentially many package elements. Names in packages should be
    /// resolved into `String` objects - this is not well defined by the specification, but matches
    /// expected behaviour of other interpreters.
    AsPackageElements,
    /// Used with [`OpInFlight::new_with`] to represent arguments that have already been resolved
    /// when an operation enters flight.
    Placeholder,
}

#[derive(Debug)]
struct OpInFlight {
    op: Opcode,
    expected_arguments: usize,
    arguments: Vec<Argument>,
    resolve_behaviour: &'static [ResolveBehaviour],
}

#[derive(Debug)]
enum Argument {
    Null,
    Object(WrappedObject),
    Namestring(AmlName),
    ByteData(u8),
    DWordData(u32),
    TrackedPc(usize),
    PkgLength(usize),
}

impl OpInFlight {
    /// Creates a new `OpInFlight`. The number of expected arguments is inferred from the number of
    /// `ResolveBehaviour`s passed.
    pub fn new(op: Opcode, resolve_behaviour: &'static [ResolveBehaviour]) -> OpInFlight {
        OpInFlight { op, expected_arguments: resolve_behaviour.len(), arguments: Vec::new(), resolve_behaviour }
    }

    /// Creates a new `OpInFlight` with the given number of expected arguments. This should be used
    /// when the correct number of expected arguments differs from the number of
    /// `ResolveBehaviour`s passed.
    pub fn new_dynamic(
        op: Opcode,
        expected_arguments: usize,
        resolve_behaviour: &'static [ResolveBehaviour],
    ) -> OpInFlight {
        OpInFlight { op, expected_arguments, arguments: Vec::new(), resolve_behaviour }
    }

    /// Creates a new `OpInFlight` with a number of arguments that have already been interpreted,
    /// and is expecting some `more` arguments.
    pub fn new_with_dynamic(
        op: Opcode,
        arguments: Vec<Argument>,
        more: usize,
        resolve_behaviour: &'static [ResolveBehaviour],
    ) -> OpInFlight {
        OpInFlight { op, expected_arguments: arguments.len() + more, arguments, resolve_behaviour }
    }

    /// Creates a new `OpInFlight` with a number of arguments that have already been interpreted,
    /// and is expecting more - the number of remaining arguments is inferred from the number of
    /// `ResolveBehaviour`s passed (with existing arguments marked as
    /// `ResolveBehaviour::Placeholder`).
    pub fn new_with(
        op: Opcode,
        arguments: Vec<Argument>,
        resolve_behaviour: &'static [ResolveBehaviour],
    ) -> OpInFlight {
        OpInFlight { op, expected_arguments: resolve_behaviour.len(), arguments, resolve_behaviour }
    }

    /// Get the desired `ResolveBehaviour` for the argument currently being interpreted
    ///
    /// LOGANFY(Fix 2): stock panics for an argument past the operator's list; return an error.
    fn resolve_behaviour(&self) -> Result<ResolveBehaviour, AmlError> {
        if let Some(behaviour) = self.resolve_behaviour.get(self.arguments.len()) {
            Ok(*behaviour)
        } else if self.op == Opcode::Package || (self.op == Opcode::VarPackage && self.arguments.len() > 0) {
            Ok(ResolveBehaviour::AsPackageElements)
        } else {
            Err(AmlError::MalformedAml("operand past the end of an operator's arguments"))
        }
    }
}

impl MethodContext {
    unsafe fn new_from_table(stream: &[u8]) -> MethodContext {
        let block = Block { stream: stream as *const [u8], pc: 0, kind: BlockKind::Table };
        MethodContext {
            current_block: block,
            block_stack: Vec::new(),
            in_flight: Vec::new(),
            args: core::array::from_fn(|_| Object::Uninitialized.wrap()),
            locals: core::array::from_fn(|_| Object::Uninitialized.wrap()),
            current_scope: AmlName::root(),
            _method: None,
        }
    }

    fn new_from_method(
        method: WrappedObject,
        args: Vec<WrappedObject>,
        scope: AmlName,
    ) -> Result<MethodContext, AmlError> {
        if let Object::Method { code, flags } = &*method {
            if args.len() != flags.arg_count() {
                return Err(AmlError::MethodArgCountIncorrect);
            }
            let block = Block {
                stream: code as &[u8] as *const [u8],
                pc: 0,
                kind: BlockKind::Method { method_scope: scope.clone() },
            };
            let args = core::array::from_fn(|i| {
                if let Some(arg) = args.get(i) { arg.clone() } else { Object::Uninitialized.wrap() }
            });
            let context = MethodContext {
                current_block: block,
                block_stack: Vec::new(),
                in_flight: Vec::new(),
                args,
                locals: core::array::from_fn(|_| Object::Uninitialized.wrap()),
                current_scope: scope,
                _method: Some(method.clone()),
            };
            Ok(context)
        } else {
            Err(AmlError::ObjectNotOfExpectedType { expected: ObjectType::Method, got: method.typ() })
        }
    }

    /// LOGANFY(Fix 1, b): the method scope this context runs in, `None` for a table. The
    /// method's own block is the bottom of the block stack.
    fn method_scope(&self) -> Option<&AmlName> {
        let bottom = self.block_stack.first().unwrap_or(&self.current_block);
        match bottom.kind {
            BlockKind::Method { ref method_scope } => Some(method_scope),
            _ => None,
        }
    }

    fn last_op(&mut self) -> Result<&mut OpInFlight, AmlError> {
        match self.in_flight.last_mut() {
            Some(op) => Ok(op),
            None => Err(AmlError::NoCurrentOp),
        }
    }

    fn contribute_arg(&mut self, arg: Argument) {
        if let Some(in_flight) = self.in_flight.last_mut()
            && in_flight.arguments.len() < in_flight.expected_arguments
        {
            in_flight.arguments.push(arg);
        }
    }

    /// Start a new `InFlightOp`.
    fn start(&mut self, op: OpInFlight) {
        trace!(
            "START OP: {:?}, args: {:?}, with {} more needed ({:?})",
            op.op,
            op.arguments,
            op.expected_arguments - op.arguments.len(),
            op.resolve_behaviour
        );
        self.in_flight.push(op);
    }

    fn retire_op(&mut self, op: OpInFlight) {
        trace!("RETIRE OP: {:?}, args: {:?}", op.op, op.arguments);
    }

    fn start_new_block(&mut self, kind: BlockKind, length: usize) -> Result<(), AmlError> {
        if self.block_stack.len() >= MAX_NESTING {
            return Err(AmlError::MalformedAml("blocks nested deeper than the interpreter's limit"));
        }
        // LOGANFY(Fix 2): a block may not run past the end of the stream it is cut from.
        if self.current_block.pc + length > self.current_block.stream.len() {
            return Err(AmlError::MalformedAml("block runs past the end of its enclosing block"));
        }
        let block = Block {
            stream: &self.current_block.stream()[..(self.current_block.pc + length)] as *const [u8],
            pc: self.current_block.pc,
            kind,
        };
        self.current_block.pc += length;
        self.block_stack.push(mem::replace(&mut self.current_block, block));
        Ok(())
    }

    fn opcode(&mut self) -> Result<Opcode, AmlError> {
        let opcode: u16 = match self.next()? {
            0x5b => {
                let ext = self.next()?;
                (0x5b << 8) as u16 | ext as u16
            }
            other => other as u16,
        };

        Ok(match opcode {
            0x00 => Opcode::Zero,
            0x01 => Opcode::One,
            0x06 => Opcode::Alias,
            0x08 => Opcode::Name,
            0x0a => Opcode::BytePrefix,
            0x0b => Opcode::WordPrefix,
            0x0c => Opcode::DWordPrefix,
            0x0d => Opcode::StringPrefix,
            0x0e => Opcode::QWordPrefix,
            0x10 => Opcode::Scope,
            0x11 => Opcode::Buffer,
            0x12 => Opcode::Package,
            0x13 => Opcode::VarPackage,
            0x14 => Opcode::Method,
            0x15 => Opcode::External,
            0x2e => Opcode::DualNamePrefix,
            0x2f => Opcode::MultiNamePrefix,
            0x30..=0x39 => Opcode::Digit(opcode as u8),    // b'0'..=b'9'
            0x41..=0x5a => Opcode::NameChar(opcode as u8), // b'A'..=b'Z'
            0x5b01 => Opcode::Mutex,
            0x5b02 => Opcode::Event,
            0x5b12 => Opcode::CondRefOf,
            0x5b13 => Opcode::CreateField,
            0x5b1f => Opcode::LoadTable,
            0x5b20 => Opcode::Load,
            0x5b21 => Opcode::Stall,
            0x5b22 => Opcode::Sleep,
            0x5b23 => Opcode::Acquire,
            0x5b24 => Opcode::Signal,
            0x5b25 => Opcode::Wait,
            0x5b26 => Opcode::Reset,
            0x5b27 => Opcode::Release,
            0x5b28 => Opcode::FromBCD,
            0x5b29 => Opcode::ToBCD,
            0x5b30 => Opcode::Revision,
            0x5b31 => Opcode::Debug,
            0x5b32 => Opcode::Fatal,
            0x5b33 => Opcode::Timer,
            0x5b80 => Opcode::OpRegion,
            0x5b81 => Opcode::Field,
            0x5b82 => Opcode::Device,
            0x5b83 => Opcode::Processor,
            0x5b84 => Opcode::PowerRes,
            0x5b85 => Opcode::ThermalZone,
            0x5b86 => Opcode::IndexField,
            0x5b87 => Opcode::BankField,
            0x5b88 => Opcode::DataRegion,
            0x5c => Opcode::RootChar,
            0x5e => Opcode::ParentPrefixChar,
            0x5f => Opcode::NameChar(b'_'),
            0x60..=0x67 => Opcode::Local(opcode as u8 - 0x60),
            0x68..=0x6e => Opcode::Arg(opcode as u8 - 0x68),
            0x70 => Opcode::Store,
            0x71 => Opcode::RefOf,
            0x72 => Opcode::Add,
            0x73 => Opcode::Concat,
            0x74 => Opcode::Subtract,
            0x75 => Opcode::Increment,
            0x76 => Opcode::Decrement,
            0x77 => Opcode::Multiply,
            0x78 => Opcode::Divide,
            0x79 => Opcode::ShiftLeft,
            0x7a => Opcode::ShiftRight,
            0x7b => Opcode::And,
            0x7c => Opcode::Nand,
            0x7d => Opcode::Or,
            0x7e => Opcode::Nor,
            0x7f => Opcode::Xor,
            0x80 => Opcode::Not,
            0x81 => Opcode::FindSetLeftBit,
            0x82 => Opcode::FindSetRightBit,
            0x83 => Opcode::DerefOf,
            0x84 => Opcode::ConcatRes,
            0x85 => Opcode::Mod,
            0x86 => Opcode::Notify,
            0x87 => Opcode::SizeOf,
            0x88 => Opcode::Index,
            0x89 => Opcode::Match,
            0x8a => Opcode::CreateDWordField,
            0x8b => Opcode::CreateWordField,
            0x8c => Opcode::CreateByteField,
            0x8d => Opcode::CreateBitField,
            0x8e => Opcode::ObjectType,
            0x8f => Opcode::CreateQWordField,
            0x90 => Opcode::LAnd,
            0x91 => Opcode::LOr,
            /*
             * `0x92` is a bit strange. It can be an opcode in its own right (`LNotOp`), but when
             * followed by `0x93..=0x95`, it instead serves as a negating prefix to encode
             * `LNotEqualOp`, `LLessEqualOp`, and `LGreaterEqualOp`.
             */
            0x92 => match self.peek() {
                Ok(0x93) => {
                    self.current_block.pc += 1;
                    Opcode::LNotEqual
                }
                Ok(0x94) => {
                    self.current_block.pc += 1;
                    Opcode::LLessEqual
                }
                Ok(0x95) => {
                    self.current_block.pc += 1;
                    Opcode::LGreaterEqual
                }
                _ => Opcode::LNot,
            },
            0x93 => Opcode::LEqual,
            0x94 => Opcode::LGreater,
            0x95 => Opcode::LLess,
            0x96 => Opcode::ToBuffer,
            0x97 => Opcode::ToDecimalString,
            0x98 => Opcode::ToHexString,
            0x99 => Opcode::ToInteger,
            0x9c => Opcode::ToString,
            0x9d => Opcode::CopyObject,
            0x9e => Opcode::Mid,
            0x9f => Opcode::Continue,
            0xa0 => Opcode::If,
            0xa1 => Opcode::Else,
            0xa2 => Opcode::While,
            0xa3 => Opcode::Noop,
            0xa4 => Opcode::Return,
            0xa5 => Opcode::Break,
            0xcc => Opcode::Breakpoint,
            0xff => Opcode::Ones,

            _ => Err(AmlError::IllegalOpcode(opcode))?,
        })
    }

    /// LOGANFY(Fix 2): the bytes left of an object whose PkgLength was read at `start_pc`. A
    /// PkgLength counts its own bytes and everything after them (ACPI 6.5 §20.2.4 Package Length
    /// Encoding), so it must cover what has been read since `start_pc` and must not run past the end
    /// of the enclosing block. Stock subtracted unchecked and sliced with the result.
    fn pkg_remaining(&self, start_pc: usize, pkg_length: usize) -> Result<usize, AmlError> {
        let consumed = self.current_block.pc - start_pc;
        let remaining = pkg_length
            .checked_sub(consumed)
            .ok_or(AmlError::MalformedAml("PkgLength shorter than the bytes it covers so far"))?;
        if self.current_block.pc + remaining > self.current_block.stream.len() {
            return Err(AmlError::MalformedAml("PkgLength runs past the end of its enclosing block"));
        }
        Ok(remaining)
    }

    fn pkglength(&mut self) -> Result<usize, AmlError> {
        let lead_byte = self.next()?;
        let byte_count = lead_byte.get_bits(6..8);
        assert!(byte_count < 4);

        if byte_count == 0 {
            Ok(lead_byte.get_bits(0..6) as usize)
        } else {
            let mut length = lead_byte.get_bits(0..4) as usize;
            for i in 0..byte_count {
                length |= (self.next()? as usize) << (4 + i * 8);
            }
            Ok(length)
        }
    }

    fn namestring(&mut self) -> Result<AmlName, AmlError> {
        use namespace::{NameComponent, NameSeg};

        /*
         * The NameString grammar is actually a little finicky and annoying.
         *
         * NameString := <RootChar NamePath> | <PrefixPath NamePath>
         * PrefixPath := Nothing | <'^' PrefixPath>
         * NamePath := NameSeg | DualNamePath | MultiNamePath | NullName
         * DualNamePath := DualNamePrefix NameSeg NameSeg
         * MultiNamePath := MultiNamePrefix SegCount NameSeg(SegCount)
         */
        const NULL_NAME: u8 = 0x00;
        const DUAL_NAME_PREFIX: u8 = 0x2e;
        const MULTI_NAME_PREFIX: u8 = 0x2f;

        let mut components = vec![];

        match self.peek()? {
            b'\\' => {
                self.next()?;
                components.push(NameComponent::Root);
            }
            b'^' => {
                components.push(NameComponent::Prefix);
                self.next()?;
                while self.peek()? == b'^' {
                    self.next()?;
                    components.push(NameComponent::Prefix);
                }
            }
            _ => (),
        }

        let next = self.next()?;
        match next {
            NULL_NAME => {}
            DUAL_NAME_PREFIX => {
                for _ in 0..2 {
                    let name_seg = [self.next()?, self.next()?, self.next()?, self.next()?];
                    components.push(NameComponent::Segment(NameSeg::from_bytes(name_seg)?));
                }
            }
            MULTI_NAME_PREFIX => {
                let count = self.next()?;
                for _ in 0..count {
                    let name_seg = [self.next()?, self.next()?, self.next()?, self.next()?];
                    components.push(NameComponent::Segment(NameSeg::from_bytes(name_seg)?));
                }
            }
            first_char => {
                if !namespace::is_lead_name_char(first_char) {
                    self.current_block.pc -= 1;
                }
                let name_seg = [first_char, self.next()?, self.next()?, self.next()?];
                components.push(namespace::NameComponent::Segment(namespace::NameSeg::from_bytes(name_seg)?));
            }
        }

        Ok(AmlName::from_components(components))
    }

    fn next(&mut self) -> Result<u8, AmlError> {
        if self.current_block.pc >= self.current_block.stream.len() {
            return Err(AmlError::RunOutOfStream);
        }

        let byte = self.current_block.stream()[self.current_block.pc];
        self.current_block.pc += 1;

        Ok(byte)
    }

    fn next_u16(&mut self) -> Result<u16, AmlError> {
        Ok(u16::from_le_bytes([self.next()?, self.next()?]))
    }

    fn next_u32(&mut self) -> Result<u32, AmlError> {
        Ok(u32::from_le_bytes([self.next()?, self.next()?, self.next()?, self.next()?]))
    }

    fn next_u64(&mut self) -> Result<u64, AmlError> {
        Ok(u64::from_le_bytes([
            self.next()?,
            self.next()?,
            self.next()?,
            self.next()?,
            self.next()?,
            self.next()?,
            self.next()?,
            self.next()?,
        ]))
    }

    fn peek(&self) -> Result<u8, AmlError> {
        if self.current_block.pc >= self.current_block.stream.len() {
            return Err(AmlError::RunOutOfStream);
        }

        Ok(self.current_block.stream()[self.current_block.pc])
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Opcode {
    Zero,
    One,
    Alias,
    Name,
    BytePrefix,
    WordPrefix,
    DWordPrefix,
    StringPrefix,
    QWordPrefix,
    Scope,
    Buffer,
    Package,
    VarPackage,
    Method,
    External,
    DualNamePrefix,
    MultiNamePrefix,
    Digit(u8),
    NameChar(u8),
    Mutex,
    Event,
    CondRefOf,
    CreateField,
    LoadTable,
    Load,
    Stall,
    Sleep,
    Acquire,
    Signal,
    Wait,
    Reset,
    Release,
    FromBCD,
    ToBCD,
    Revision,
    Debug,
    Fatal,
    Timer,
    OpRegion,
    Field,
    Device,
    Processor,
    PowerRes,
    ThermalZone,
    IndexField,
    BankField,
    DataRegion,
    RootChar,
    ParentPrefixChar,
    Local(u8),
    Arg(u8),
    Store,
    RefOf,
    Add,
    Concat,
    Subtract,
    Increment,
    Decrement,
    Multiply,
    Divide,
    ShiftLeft,
    ShiftRight,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Not,
    FindSetLeftBit,
    FindSetRightBit,
    DerefOf,
    ConcatRes,
    Mod,
    Notify,
    SizeOf,
    Index,
    Match,
    CreateDWordField,
    CreateWordField,
    CreateByteField,
    CreateBitField,
    ObjectType,
    CreateQWordField,
    LAnd,
    LOr,
    LNot,
    LNotEqual,
    LLessEqual,
    LGreaterEqual,
    LEqual,
    LGreater,
    LLess,
    ToBuffer,
    ToDecimalString,
    ToHexString,
    ToInteger,
    ToString,
    CopyObject,
    Mid,
    Continue,
    If,
    Else,
    While,
    Noop,
    Return,
    Break,
    Breakpoint,
    Ones,

    /*
     * Internal opcodes are not produced from the bytecode, but are used to track special in-flight
     * ops etc.
     */
    InternalMethodCall,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Operation {
    Mid,
    SizeOf,
    Acquire,
    Release,
    ConvertToBuffer,

    ToBuffer,
    ToInteger,
    ToString,
    ToDecOrHexString,

    ReadBufferField,
    WriteBufferField,
    LogicalOp,
    DecodePrt,
    ParseResource,

    ResetEvent,
    SignalEvent,
    WaitEvent,
}

/// LOGANFY(Fix 3, 1, review): ACPICA's implicit conversion of a compare operand to an Integer
/// (`exconvrt.c` `AcpiExConvertToInteger` with `ACPI_IMPLICIT_CONVERSION`). A Buffer gives its
/// first `int_bytes` bytes, little-endian (`exconvrt.c:123-152`), and a zero-length Buffer is an
/// error (`exconvrt.c:127-130`, AE_AML_BUFFER_LIMIT). A String is read as hex
/// (`exconvrt.c:113-116`, `implicit_strtoul64`).
fn implicit_to_integer(object: &Object, int_bytes: usize) -> Result<u64, AmlError> {
    match object {
        Object::Integer(value) => Ok(*value),
        Object::Buffer(bytes) if bytes.is_empty() => {
            Err(AmlError::InvalidOperationOnObject { op: Operation::LogicalOp, typ: ObjectType::Buffer })
        }
        Object::Buffer(_) => object.to_integer(int_bytes),
        Object::String(value) => Ok(implicit_strtoul64(value.as_bytes(), int_bytes)),
        _ => Err(AmlError::InvalidOperationOnObject { op: Operation::LogicalOp, typ: object.typ() }),
    }
}

/// LOGANFY(Fix 3, 1, review): `AcpiUtImplicitStrtoul64` (`utstrtoul64.c:229-259`). Leading
/// whitespace (`utclib.c:898-921` `_ACPI_SP`: 0x09-0x0D and space), then an optional `0x`/`0X`
/// prefix, then leading zeros are skipped (`utstrsuppt.c` `AcpiUtRemoveWhitespace`,
/// `AcpiUtRemoveHexPrefix`, `AcpiUtRemoveLeadingZeros`). Hex digits accumulate until the first
/// other character or the NUL. A digit that would overflow the integer width ends the conversion
/// and keeps the value so far (`utstrsuppt.c` `AcpiUtConvertHexString`, `AcpiUtInsertDigit`,
/// `AcpiUtStrtoulMultiply64`, `AcpiUtStrtoulAdd64`). There is no error case.
fn implicit_strtoul64(string: &[u8], int_bytes: usize) -> u64 {
    let max = if int_bytes == 4 { u64::from(u32::MAX) } else { u64::MAX };
    let end = string.iter().position(|&b| b == 0).unwrap_or(string.len());
    let mut s = &string[..end];
    while let [b'\t'..=b'\r' | b' ', rest @ ..] = s {
        s = rest;
    }
    if let [b'0', b'x' | b'X', rest @ ..] = s {
        s = rest;
    }
    while let [b'0', rest @ ..] = s {
        s = rest;
    }
    let mut value = 0u64;
    for &c in s {
        let Some(digit) = (c as char).to_digit(16) else { break };
        match value.checked_mul(16).filter(|&v| v <= max).and_then(|v| v.checked_add(u64::from(digit))) {
            Some(next) if next <= max => value = next,
            _ => break,
        }
    }
    value
}

/// LOGANFY(Fix 3, 1, review): ACPICA's implicit conversion of a compare operand to a String
/// (`exconvrt.c` `AcpiExConvertToString` with `ACPI_IMPLICIT_CONVERT_HEX`). An Integer becomes
/// two upper-case hex digits per integer byte with leading zeros kept (`exconvrt.c:473-479`, the
/// `default` arm, then `AcpiExConvertToAscii`; `uthex.c:23-26`). A Buffer becomes `0xNN` per
/// byte, separated by spaces (`exconvrt.c:556-569` and the loop after it); a zero-length Buffer
/// gives an empty String.
fn implicit_to_hex_string(object: &Object, int_bytes: usize) -> Result<String, AmlError> {
    match object {
        Object::String(value) => Ok(value.clone()),
        Object::Integer(value) if int_bytes == 4 => Ok(alloc::format!("{:08X}", *value as u32)),
        Object::Integer(value) => Ok(alloc::format!("{value:016X}")),
        Object::Buffer(bytes) => {
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            Ok(parts.join(" "))
        }
        _ => Err(AmlError::InvalidOperationOnObject { op: Operation::LogicalOp, typ: object.typ() }),
    }
}

#[derive(Clone, PartialEq, Debug)]
#[non_exhaustive]
pub enum AmlError {
    RunOutOfStream,
    IllegalOpcode(u16),
    InvalidFieldFlags,

    InvalidName(Option<AmlName>),

    InvalidNameSeg([u8; 4]),
    InvalidNormalizedName(AmlName),
    RootHasNoParent,
    EmptyNamesAreInvalid,
    LevelDoesNotExist(AmlName),
    NameCollision(AmlName),
    ObjectDoesNotExist(AmlName),

    NoCurrentOp,
    ElseFoundWithoutCorrespondingIf,
    ContinueOutsideOfWhile,
    BreakOutsideOfWhile,

    MethodArgCountIncorrect,

    InvalidOperationOnObject {
        op: Operation,
        typ: ObjectType,
    },
    IndexOutOfBounds,
    ObjectNotOfExpectedType {
        expected: ObjectType,
        got: ObjectType,
    },

    InvalidResourceDescriptor,
    UnexpectedResourceType,

    NoHandlerForRegionAccess(RegionSpace),
    MutexAcquireTimeout,

    PrtInvalidAddress,
    PrtInvalidPin,
    PrtInvalidGsi,
    PrtInvalidSource,
    PrtNoEntry,

    /// This is emitted to signal that the library does not support the requested behaviour. This
    /// should eventually never be emitted.
    LibUnimplemented,
    /// LOGANFY(Fix 2): the AML byte stream is malformed in a way the interpreter detected (the
    /// string names the check). Stock code panicked or indexed out of bounds on these.
    MalformedAml(&'static str),
}
