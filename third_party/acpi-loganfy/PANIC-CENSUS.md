<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `acpi-loganfy` panic census (OT-86 Fix 2)

Every explicit panic site in `src/aml`, before and after Fix 2. Scope is `src/aml` only; the rest of the crate
(`sdt/`, `platform/`, `registers.rs`, `address.rs`, `rsdp.rs`, `lib.rs`) is out of scope for Fix 2.

**What counts as a site.** A non-comment source line outside a `#[cfg(test)] mod tests` that contains `panic!`,
`.unwrap()`, `.expect(`, `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!` or
`debug_assert`. `tools/acpi-aml-grade/tests/panic_census.rs` scans the crate the same way and fails if Part A
does not list exactly the sites it finds, by file, line and text. Change the crate, and this file changes with it.

**Implicit panics** (slice indexing, integer overflow, division, allocation failure, unbounded recursion) have no
marker to scan for, so they are covered by class rather than by line; see "Implicit panics" below.

## Summary

- Stock `acpi` 6.1.1 (crates.io): **181** explicit sites in `src/aml`.
- Converted to an error: **124**. Kept: **57** (Part A).
- Part A, the 58 sites in the patched crate: **47** proven unreachable, **11** host-API only, **0** unclassified.
  (44 of the Part A lines are stock lines kept byte-for-byte; the rest are stock `panic!()`/`unreachable!()` lines rewritten as `unreachable!("reason")`, plus one Fix 1 `unwrap`.)

"Proven unreachable" means no AML byte stream can reach it; the argument is in the row. "Host-API only" means only
a host calling the crate's public API in a way the interpreter never does can reach it. The kernel would be that host
(OT-86 L5), so these are preconditions on our own call sites, not on firmware.

## Part A: the sites that remain

| Site | Code | Disposition | Why |
|---|---|---|---|
| `mod.rs:272` | `assert!(handlers.get(&space).is_none(), "Tried to install handler for same space twice!");` | host-API | `install_region_handler` is called by the host, never by AML; installing two handlers for one space is a host programming error |
| `mod.rs:283` | `if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_INI").unwrap(), vec![]) {` | unreachable | a constant: the NameString literal parses |
| `mod.rs:286` | `if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_SB._INI").unwrap(), vec![]) {` | unreachable | a constant: the NameString literal parses |
| `mod.rs:316` | `.evaluate_if_present(AmlName::from_str("_STA").unwrap().resolve(path)?, vec![])` | unreachable | a constant: the NameString literal parses |
| `mod.rs:335` | `self.evaluate_if_present(AmlName::from_str("_INI").unwrap().resolve(path)?, vec![])` | unreachable | a constant: the NameString literal parses |
| `mod.rs:464` | `let caller = stack.pop().unwrap();` | unreachable | inside `while stack.len() > base` (Fix 1, b), so the stack is not empty |
| `mod.rs:554` | `_ => unreachable!(),` | unreachable | the enclosing match arm admits only the opcodes this match lists (Increment or Decrement at one such line, Device or ThermalZone at the other) |
| `mod.rs:936` | `_ => unreachable!("this arm only matches the Create*Field opcodes"),` | unreachable | the enclosing match arm admits only the Create*Field opcodes this match lists |
| `mod.rs:1180` | `ObjectType::Reference => unreachable!("typ() never returns Reference"),` | unreachable | `Object::typ` maps a Reference to its inner object's type, so it never returns `Reference` |
| `mod.rs:1183` | `ObjectType::RawDataBuffer => unreachable!("no RawDataBuffer is ever constructed"),` | host-API | the interpreter never constructs `Object::RawDataBuffer`; only a host can (a `NativeMethod` result or an `evaluate` argument) |
| `mod.rs:1689` | `_ => unreachable!(),` | unreachable | the enclosing match arm admits only the opcodes this match lists (Increment or Decrement at one such line, Device or ThermalZone at the other) |
| `mod.rs:2023` | `Opcode::InternalMethodCall => unreachable!("InternalMethodCall is never decoded"),` | unreachable | `MethodContext::opcode` never yields `InternalMethodCall`; the interpreter starts it itself |
| `mod.rs:2164` | `_ => unreachable!("do_binary_maths is only called for these opcodes"),` | unreachable | the dispatcher calls this function only for the opcodes the match lists |
| `mod.rs:2214` | `_ => unreachable!("do_unary_maths is only called for these opcodes"),` | unreachable | the dispatcher calls this function only for the opcodes the match lists |
| `mod.rs:2299` | `_ => unreachable!("do_logical_op is only called for these opcodes"),` | unreachable | the dispatcher calls this function only for the opcodes the match lists |
| `mod.rs:2404` | `let mut buffer = source.split_inclusive(\|b\| *b == b'\0').next().unwrap();` | unreachable | the `else` branch of `source.is_empty()`: `split_inclusive` of a non-empty slice yields at least one item |
| `mod.rs:2434` | `_ => unreachable!("do_to_dec_hex_string is only called for these opcodes"),` | unreachable | the dispatcher calls this function only for the opcodes the match lists |
| `mod.rs:2445` | `_ => unreachable!("do_to_dec_hex_string is only called for these opcodes"),` | unreachable | the dispatcher calls this function only for the opcodes the match lists |
| `mod.rs:2539` | `Object::RawDataBuffer => unreachable!("no RawDataBuffer is ever constructed"),` | host-API | the interpreter never constructs `Object::RawDataBuffer`; only a host can (a `NativeMethod` result or an `evaluate` argument) |
| `mod.rs:2807` | `ReferenceKind::RefOf \| ReferenceKind::Index => unreachable!(),` | unreachable | stores through `RefOf`/`Index` return earlier in `do_store` (Fix 1, d) |
| `mod.rs:3301` | `let seg = match self.evaluate_if_present(AmlName::from_str("_SEG").unwrap().resolve(path)?, vec![])? {` | unreachable | a constant: the NameString literal parses |
| `mod.rs:3305` | `let bus = match self.evaluate_if_present(AmlName::from_str("_BBR").unwrap().resolve(path)?, vec![])? {` | unreachable | a constant: the NameString literal parses |
| `mod.rs:3310` | `let adr = self.evaluate_if_present(AmlName::from_str("_ADR").unwrap().resolve(path)?, vec![])?;` | unreachable | a constant: the NameString literal parses |
| `mod.rs:3736` | `assert!(byte_count < 4);` | unreachable | `byte_count` is `lead_byte.get_bits(6..8)`, a 2-bit value |
| `namespace.rs:30` | `namespace.add_level(AmlName::from_str("\\_GPE").unwrap(), NamespaceLevelKind::Scope).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:31` | `namespace.add_level(AmlName::from_str("\\_SB").unwrap(), NamespaceLevelKind::Scope).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:32` | `namespace.add_level(AmlName::from_str("\\_SI").unwrap(), NamespaceLevelKind::Scope).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:33` | `namespace.add_level(AmlName::from_str("\\_PR").unwrap(), NamespaceLevelKind::Scope).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:34` | `namespace.add_level(AmlName::from_str("\\_TZ").unwrap(), NamespaceLevelKind::Scope).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:38` | `AmlName::from_str("\\_GL").unwrap(),` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:41` | `.unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:52` | `.insert(AmlName::from_str("\\_OS").unwrap(), Object::String("Microsoft Windows NT".to_string()).wrap())` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:53` | `.unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:68` | `AmlName::from_str("\\_OSI").unwrap(),` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:127` | `.unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:135` | `namespace.insert(AmlName::from_str("\\_REV").unwrap(), Object::Integer(2).wrap()).unwrap();` | unreachable | a constant: the NameString literal parses, and the fresh namespace `Namespace::new` builds has no level or name it could collide with |
| `namespace.rs:141` | `assert!(path.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:159` | `assert!(path.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:173` | `assert!(path.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:191` | `assert!(path.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:202` | `assert!(path.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:223` | `assert!(scope.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:260` | `assert!(scope.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `namespace.rs:554` | `name.pop().unwrap();` | unreachable | guarded by the length check immediately before it |
| `namespace.rs:575` | `Some(NameComponent::Prefix) => unreachable!(), // Prefix chars are removed by normalization` | unreachable | `normalize` removes every prefix component before this match |
| `namespace.rs:582` | `assert!(scope.is_absolute());` | host-API | every interpreter call passes a name from `AmlName::resolve` (absolute) or the current scope; a relative path can only come from the host (`evaluate`, `pci_routing`) |
| `object.rs:251` | `_ => unreachable!("to_buffer is called with the integer width, 4 or 8"),` | unreachable | the interpreter's only caller passes 4 or 8 (`Concat`); a host could pass another width |
| `object.rs:536` | `assert!(align == 0 \|\| align.is_power_of_two());` | unreachable | both callers pass the access width in bits, 8 / 16 / 32 / 64 |
| `pci_routing.rs:179` | `let path = AmlName::from_str("_CRS").unwrap().resolve(name)?;` | unreachable | a constant: the NameString literal parses |
| `resource.rs:115` | `0x80..=0xff => unreachable!("descriptor_type is 7 bits"),` | unreachable | the descriptor type is `get_bits` of 7 (large) or 4 (small) bits, and every value of that width has an arm |
| `resource.rs:161` | `0x10..=0xFF => unreachable!("descriptor_type is 4 bits"),` | unreachable | the descriptor type is `get_bits` of 7 (large) or 4 (small) bits, and every value of that width has an arm |
| `resource.rs:315` | `let granularity = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | unreachable | `bytes.len() >= 6 + size * 5` is checked at the top, so `chunks_exact(size)` over `bytes[6..]` has five chunks |
| `resource.rs:316` | `let address_range_min = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | unreachable | `bytes.len() >= 6 + size * 5` is checked at the top, so `chunks_exact(size)` over `bytes[6..]` has five chunks |
| `resource.rs:317` | `let address_range_max = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | unreachable | `bytes.len() >= 6 + size * 5` is checked at the top, so `chunks_exact(size)` over `bytes[6..]` has five chunks |
| `resource.rs:318` | `let translation_offset = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | unreachable | `bytes.len() >= 6 + size * 5` is checked at the top, so `chunks_exact(size)` over `bytes[6..]` has five chunks |
| `resource.rs:319` | `let length = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | unreachable | `bytes.len() >= 6 + size * 5` is checked at the top, so `chunks_exact(size)` over `bytes[6..]` has five chunks |
| `resource.rs:484` | `_ => unreachable!(),` | unreachable | matches `get_bits` of a 2-bit field and the arms cover 0-3 |
| `resource.rs:492` | `_ => unreachable!(),` | unreachable | matches `get_bits` of a 2-bit field and the arms cover 0-3 |

## Part B: every stock site and what replaced it

Stock line numbers are those of the crates.io 6.1.1 tarball (`~/.cargo/registry/src/*/acpi-6.1.1/src/aml`). "Fail
closed" is `fail_closed(feature)`: a `LOGANFY fail-closed: <feature>` warn line and `AmlError::LibUnimplemented`,
never a made-up value. Each `MalformedAml` check and each fail-closed class has a KAT in
`tools/acpi-aml-grade/tests/fix2_kats.rs` (or `fix1_kats.rs` for the Fix 1 items), listed in the Fix 2 findings.

| Stock site | Stock code | Now |
|---|---|---|
| `mod.rs:209` | `assert!(handlers.get(&space).is_none(), "Tried to install handler for same space twice!");` | kept: Part A |
| `mod.rs:220` | `if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_INI").unwrap(), vec![]) {` | kept: Part A |
| `mod.rs:223` | `if let Err(err) = self.evaluate_if_present(AmlName::from_str("\\_SB._INI").unwrap(), vec![]) {` | kept: Part A |
| `mod.rs:253` | `.evaluate_if_present(AmlName::from_str("_STA").unwrap().resolve(path)?, vec![])` | kept: Part A |
| `mod.rs:256` | `let Object::Integer(result) = *result else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:270` | `self.evaluate_if_present(AmlName::from_str("_INI").unwrap().resolve(path)?, vec![])` | kept: Part A |
| `mod.rs:347` | `self.registers.pm1_control_registers.set_bit(Pm1ControlBit::GlobalLockRelease, true).unwrap();` | fail closed: PM1 GlobalLockRelease write |
| `mod.rs:421` | `let [Argument::Object(operand)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:433` | `_ => unreachable!(),` | kept: Part A |
| `mod.rs:461` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:481` | `panic!();` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:496` | `panic!();` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:511` | `panic!();` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:568` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:579` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:593` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:616` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:643` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:661` | `let Argument::Object(object) = arg else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:676` | `assert_eq!(context.current_block.kind, BlockKind::Package);` | MalformedAml("Package completed outside its own block") |
| `mod.rs:677` | `assert_eq!(context.peek(), Err(AmlError::RunOutOfStream));` | MalformedAml("Package completed outside its own block") |
| `mod.rs:678` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("Package block without an enclosing block") |
| `mod.rs:683` | `let Argument::Object(total_elements) = &op.arguments[0] else { panic!() };` | MalformedAml("VarPackage without its NumElements") |
| `mod.rs:689` | `let Argument::Object(object) = arg else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:697` | `assert_eq!(context.current_block.kind, BlockKind::VarPackage);` | MalformedAml("VarPackage completed outside its own block") |
| `mod.rs:698` | `assert_eq!(context.peek(), Err(AmlError::RunOutOfStream));` | MalformedAml("VarPackage completed outside its own block") |
| `mod.rs:699` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("VarPackage block without an enclosing block") |
| `mod.rs:710` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:744` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:754` | `_ => unreachable!(),` | kept: Part A |
| `mod.rs:766` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:784` | `let [Argument::Object(object), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:789` | `let [Argument::Object(object)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:796` | `let [Argument::Object(object), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:809` | `let [Argument::Object(object)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:813` | `let path = AmlName::from_str(&object.as_string().unwrap())?` | Fix 1 (G2): DerefOf matches the String instead of unwrapping it |
| `mod.rs:826` | `let [Argument::Object(msec)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:831` | `let [Argument::Object(usec)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:836` | `let [Argument::Object(mutex)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:852` | `let [Argument::Object(mutex)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:869` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:878` | `panic!();` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:897` | `panic!();` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:901` | `let [Argument::Object(object)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:917` | `let [Argument::Object(object)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:938` | `ObjectType::Reference => panic!(),` | unreachable!: typ() never returns Reference (Part A) |
| `mod.rs:939` | `ObjectType::RawDataBuffer => todo!(),` | unreachable!: no RawDataBuffer is ever constructed (Part A) |
| `mod.rs:956` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:977` | `let [Argument::Object(predicate)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:982` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:986` | `_ => panic!("Unexpected operation has created in-flight op!"),` | fail closed: the opcode's name (`fail_closed(&format!("{other:?}"))`), e.g. Notify |
| `mod.rs:1020` | `assert!(!context.block_stack.is_empty());` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1021` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1036` | `assert!(!context.block_stack.is_empty());` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1052` | `assert!(!context.block_stack.is_empty());` | MalformedAml("VarPackage block without an enclosing block") |
| `mod.rs:1059` | `panic!()` | MalformedAml("VarPackage without its NumElements") |
| `mod.rs:1079` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1166` | `.unwrap(),` | MalformedAml("String literal that is not ASCII/UTF-8") |
| `mod.rs:1269` | `Opcode::LoadTable => todo!(),` | fail closed: LoadTable |
| `mod.rs:1270` | `Opcode::Load => todo!(),` | fail closed: Load |
| `mod.rs:1391` | `_ => unreachable!(),` | kept: Part A |
| `mod.rs:1541` | `panic!("Invalid resolve behaviour for name to be resolved!")` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1598` | `Opcode::Match => todo!(),` | fail closed: Match |
| `mod.rs:1632` | `Opcode::CopyObject => todo!(),` | fail closed: CopyObject |
| `mod.rs:1680` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1687` | `context.current_block = context.block_stack.pop().unwrap();` | MalformedAml("block structure out of step with the stream") |
| `mod.rs:1702` | `Opcode::InternalMethodCall => panic!(),` | unreachable!: InternalMethodCall is never decoded (Part A) |
| `mod.rs:1736` | `todo!()` | parse-only: later fields in the list are marked `access_unimplemented` and fail closed when read or written |
| `mod.rs:1741` | `todo!("Connect field :(");` | Fix 1 (c): ConnectField is parsed; later fields fail closed |
| `mod.rs:1744` | `todo!("Extended access field :(");` | Fix 1 (c): ExtendedAccessField is parsed; later fields fail closed |
| `mod.rs:1769` | `let [Argument::Object(left), Argument::Object(right), target] = &op.arguments[0..3] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1793` | `_ => panic!(),` | unreachable!: do_binary_maths is only called for these opcodes (Part A) |
| `mod.rs:1804` | `let [Argument::Object(operand)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1837` | `_ => panic!(),` | unreachable!: do_unary_maths is only called for these opcodes (Part A) |
| `mod.rs:1847` | `let [Argument::Object(operand)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1856` | `let [Argument::Object(left), Argument::Object(right)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1886` | `let Object::Buffer(ref right) = *right else { panic!() };` | Fix 3 (1): the right operand is converted to a Buffer as ACPICA does; one that cannot be is `InvalidOperationOnObject` |
| `mod.rs:1911` | `_ => panic!(),` | unreachable!: do_logical_op is only called for these opcodes (Part A) |
| `mod.rs:1921` | `let [Argument::Object(operand), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1954` | `let [Argument::Object(operand), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:1999` | `let [Argument::Object(source), Argument::Object(length), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2007` | `let mut buffer = source.split_inclusive(\|b\| *b == b'\0').next().unwrap();` | kept: Part A |
| `mod.rs:2027` | `let [Argument::Object(operand), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2035` | `_ => panic!(),` | unreachable!: do_to_dec_hex_string is only called for these opcodes (Part A) |
| `mod.rs:2046` | `_ => panic!(),` | unreachable!: do_to_dec_hex_string is only called for these opcodes (Part A) |
| `mod.rs:2071` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2106` | `let [Argument::Object(source1), Argument::Object(source2), target] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2127` | `Object::RawDataBuffer => todo!(),` | unreachable!: no RawDataBuffer is ever constructed (Part A) |
| `mod.rs:2167` | `let [Argument::Object(value)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2184` | `let [Argument::Object(value)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2201` | `let [Argument::Object(object)] = &op.arguments[..] else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2218` | `panic!()` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2313` | `_ => panic!(),` | fail closed: Store of this object type to a buffer field |
| `mod.rs:2318` | `ReferenceKind::RefOf => todo!(),` | Fix 1 (d): a store through `RefOf` is implemented; this arm is now `unreachable!` (Part A) |
| `mod.rs:2333` | `ReferenceKind::Unresolved => todo!(),` | MalformedAml("store to a name that does not exist") |
| `mod.rs:2339` | `_ => panic!("Stores to objects like {:?} are not yet supported", target),` | Fix 3 (8): stores to a named Buffer or String are built (ACPICA `exstorob.c`); a source that is not converted fails closed (Store of this object type to a Buffer/String); other target types fail closed: Store to a named object of this type |
| `mod.rs:2342` | `Argument::Namestring(_) => todo!(),` | MalformedAml("store to a non-object target") |
| `mod.rs:2344` | `panic!()` | MalformedAml("store to a non-object target") |
| `mod.rs:2382` | `todo!();` | fail closed: BankField read |
| `mod.rs:2387` | `todo!();` | fail closed: IndexField read |
| `mod.rs:2390` | `let Object::OpRegion(ref read_region) = **read_region else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2442` | `todo!();` | fail closed: BankField write |
| `mod.rs:2447` | `todo!();` | fail closed: IndexField write |
| `mod.rs:2450` | `let Object::OpRegion(ref write_region) = **write_region else { panic!() };` | MalformedAml("operator arguments of an unexpected kind") |
| `mod.rs:2516` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2525` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2534` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2580` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2590` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2600` | `_ => panic!(),` | MalformedAml("an access width this region space cannot do") |
| `mod.rs:2629` | `let seg = match self.evaluate_if_present(AmlName::from_str("_SEG").unwrap().resolve(path)?, vec![])? {` | kept: Part A |
| `mod.rs:2633` | `let bus = match self.evaluate_if_present(AmlName::from_str("_BBR").unwrap().resolve(path)?, vec![])? {` | kept: Part A |
| `mod.rs:2638` | `let adr = self.evaluate_if_present(AmlName::from_str("_ADR").unwrap().resolve(path)?, vec![])?;` | kept: Part A |
| `mod.rs:2794` | `panic!("Tried to get resolving behaviour for unexpected argument for operation of type {:?}", self.op);` | MalformedAml("operand past the end of an operator's arguments") |
| `mod.rs:3029` | `assert!(byte_count < 4);` | kept: Part A |
| `namespace.rs:29` | `namespace.add_level(AmlName::from_str("\\_GPE").unwrap(), NamespaceLevelKind::Scope).unwrap();` | kept: Part A |
| `namespace.rs:30` | `namespace.add_level(AmlName::from_str("\\_SB").unwrap(), NamespaceLevelKind::Scope).unwrap();` | kept: Part A |
| `namespace.rs:31` | `namespace.add_level(AmlName::from_str("\\_SI").unwrap(), NamespaceLevelKind::Scope).unwrap();` | kept: Part A |
| `namespace.rs:32` | `namespace.add_level(AmlName::from_str("\\_PR").unwrap(), NamespaceLevelKind::Scope).unwrap();` | kept: Part A |
| `namespace.rs:33` | `namespace.add_level(AmlName::from_str("\\_TZ").unwrap(), NamespaceLevelKind::Scope).unwrap();` | kept: Part A |
| `namespace.rs:37` | `AmlName::from_str("\\_GL").unwrap(),` | kept: Part A |
| `namespace.rs:40` | `.unwrap();` | kept: Part A |
| `namespace.rs:51` | `.insert(AmlName::from_str("\\_OS").unwrap(), Object::String("Microsoft Windows NT".to_string()).wrap())` | kept: Part A |
| `namespace.rs:52` | `.unwrap();` | kept: Part A |
| `namespace.rs:67` | `AmlName::from_str("\\_OSI").unwrap(),` | kept: Part A |
| `namespace.rs:126` | `.unwrap();` | kept: Part A |
| `namespace.rs:134` | `namespace.insert(AmlName::from_str("\\_REV").unwrap(), Object::Integer(2).wrap()).unwrap();` | kept: Part A |
| `namespace.rs:140` | `assert!(path.is_absolute());` | kept: Part A |
| `namespace.rs:158` | `assert!(path.is_absolute());` | kept: Part A |
| `namespace.rs:172` | `assert!(path.is_absolute());` | kept: Part A |
| `namespace.rs:190` | `assert!(path.is_absolute());` | kept: Part A |
| `namespace.rs:201` | `assert!(path.is_absolute());` | kept: Part A |
| `namespace.rs:222` | `assert!(scope.is_absolute());` | kept: Part A |
| `namespace.rs:259` | `assert!(scope.is_absolute());` | kept: Part A |
| `namespace.rs:284` | `assert_ne!(*path, AmlName::root());` | MalformedAml("the root scope used as an object name") |
| `namespace.rs:286` | `let (last_seg, levels) = path.0[1..].split_last().unwrap();` | MalformedAml("an object name without segments") |
| `namespace.rs:288` | `panic!();` | MalformedAml("an object name that is not normalised") |
| `namespace.rs:299` | `panic!();` | MalformedAml("an object name that is not normalised") |
| `namespace.rs:311` | `assert_ne!(*path, AmlName::root());` | MalformedAml("the root scope used as an object name") |
| `namespace.rs:313` | `let (last_seg, levels) = path.0[1..].split_last().unwrap();` | MalformedAml("an object name without segments") |
| `namespace.rs:315` | `panic!();` | MalformedAml("an object name that is not normalised") |
| `namespace.rs:328` | `panic!();` | MalformedAml("an object name that is not normalised") |
| `namespace.rs:532` | `name.pop().unwrap();` | kept: Part A |
| `namespace.rs:553` | `Some(NameComponent::Prefix) => unreachable!(), // Prefix chars are removed by normalization` | kept: Part A |
| `namespace.rs:560` | `assert!(scope.is_absolute());` | kept: Part A |
| `object.rs:225` | `_ => panic!(),` | unreachable!: to_buffer is called with the integer width, 4 or 8 (Part A) |
| `object.rs:237` | `_ => panic!(),` | InvalidOperationOnObject |
| `object.rs:255` | `_ => panic!(),` | InvalidOperationOnObject |
| `object.rs:362` | `_ => unreachable!(),` | InvalidFieldFlags |
| `object.rs:484` | `assert!(align == 0 \|\| align.is_power_of_two());` | kept: Part A |
| `pci_routing.rs:178` | `let path = AmlName::from_str("_CRS").unwrap().resolve(name)?;` | kept: Part A |
| `resource.rs:76` | `0x01 => unimplemented!("24-bit Memory Range Descriptor"),` | fail closed: 24-bit Memory Range Descriptor |
| `resource.rs:77` | `0x02 => unimplemented!("Generic Register Descriptor"),` | fail closed: Generic Register Descriptor |
| `resource.rs:78` | `0x03 => unimplemented!("0x03 Reserved"),` | fail closed: 0x03 Reserved |
| `resource.rs:79` | `0x04 => unimplemented!("Vendor-defined Descriptor"),` | fail closed: Vendor-defined Descriptor |
| `resource.rs:80` | `0x05 => unimplemented!("32-bit Memory Range Descriptor"),` | fail closed: 32-bit Memory Range Descriptor |
| `resource.rs:86` | `0x0b => unimplemented!("Extended Address Space Descriptor"),` | fail closed: Extended Address Space Descriptor |
| `resource.rs:87` | `0x0c => unimplemented!("GPIO Connection Descriptor"),` | fail closed: GPIO Connection Descriptor |
| `resource.rs:88` | `0x0d => unimplemented!("Pin Function Descriptor"),` | fail closed: Pin Function Descriptor |
| `resource.rs:89` | `0x0e => unimplemented!("GenericSerialBus Connection Descriptor"),` | fail closed: GenericSerialBus Connection Descriptor |
| `resource.rs:90` | `0x0f => unimplemented!("Pin Configuration Descriptor"),` | fail closed: Pin Configuration Descriptor |
| `resource.rs:91` | `0x10 => unimplemented!("Pin Group Descriptor"),` | fail closed: Pin Group Descriptor |
| `resource.rs:92` | `0x11 => unimplemented!("Pin Group Function Descriptor"),` | fail closed: Pin Group Function Descriptor |
| `resource.rs:93` | `0x12 => unimplemented!("Pin Group Configuration Descriptor"),` | fail closed: Pin Group Configuration Descriptor |
| `resource.rs:96` | `0x80..=0xff => unreachable!(),` | unreachable!: descriptor_type is 7 bits (Part A) |
| `resource.rs:130` | `0x06 => unimplemented!("Start Dependent Functions Descriptor"),` | fail closed: Start Dependent Functions Descriptor |
| `resource.rs:131` | `0x07 => unimplemented!("End Dependent Functions Descriptor"),` | fail closed: End Dependent Functions Descriptor |
| `resource.rs:133` | `0x09 => unimplemented!("Fixed Location IO Port Descriptor"),` | fail closed: Fixed Location IO Port Descriptor |
| `resource.rs:134` | `0x0A => unimplemented!("Fixed DMA Descriptor"),` | fail closed: Fixed DMA Descriptor |
| `resource.rs:136` | `0x0E => unimplemented!("Vendor Defined Descriptor"),` | fail closed: Vendor Defined Descriptor |
| `resource.rs:138` | `0x10..=0xFF => unreachable!(),` | unreachable!: descriptor_type is 4 bits (Part A) |
| `resource.rs:276` | `192..=255 => unimplemented!(),` | fail closed: vendor-defined Address Space resource type |
| `resource.rs:291` | `let granularity = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | kept: Part A |
| `resource.rs:292` | `let address_range_min = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | kept: Part A |
| `resource.rs:293` | `let address_range_max = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | kept: Part A |
| `resource.rs:294` | `let translation_offset = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | kept: Part A |
| `resource.rs:295` | `let length = LittleEndian::read_uint(address_fields.next().unwrap(), size);` | kept: Part A |
| `resource.rs:460` | `_ => unreachable!(),` | kept: Part A |
| `resource.rs:467` | `3 => unimplemented!("Reserved DMA transfer type preference"),` | fail closed: Reserved DMA transfer type preference |
| `resource.rs:468` | `_ => unreachable!(),` | kept: Part A |
| `resource.rs:538` | `assert_eq!(number_of_interrupts, 1);` | fail closed: Extended Interrupt Descriptor without exactly one interrupt |

## Implicit panics

These have no marker, so the census covers them by class. Each class lists the check that removes it and a KAT.
The fuzz run (below, and findings §2) is the evidence that no site of these classes is left on the paths the corpus
and its mutations reach. It is evidence, not proof: the review dispatch asks for a check of the claims.

| Class | Where AML controls the value | What Fix 2 does | KAT (`fix2_kats.rs`) |
|---|---|---|---|
| PkgLength past or short of its block | every PkgLength-delimited object (Method, Scope, Device, If/While/Else, Buffer, Package, Field…) | all go through `MethodContext::pkg_remaining` (`checked_sub` + a bound against the enclosing block) and `start_new_block` (checks the length) | `pkglength_past_the_end_of_its_block`, `pkglength_shorter_than_its_header` |
| Buffer sized or initialised by AML | `Buffer (n) {…}` | `n <= MAX_OBJECT_BYTES`; the initializer is no longer than `n` | `buffer_initializer_longer_than_its_size`, `buffer_size_over_the_object_limit` |
| Package/VarPackage counts | NumElements, VarNumElements | count `<= MAX_OBJECT_BYTES`; the count operand must exist; surplus initializers are an error | `package_with_surplus_initializers`, `varpackage_without_its_count` |
| Buffer-field bounds | `Create*Field` index and length; reads and writes of the field | `checked_mul` for `index * 8`; `check_buffer_field` (a Buffer source, length > 0, end within the buffer); `read_buffer_field`/`write_buffer_field` re-check | `create_field_outside_its_buffer` |
| String slicing | `Mid`, `ToString`, `Concat` | `Mid` clips to the source and refuses a cut inside a UTF-8 character | `mid_past_the_end_returns_the_rest` |
| Division | `Divide`, `Mod` | a zero divisor is an error | `divide_and_mod_by_zero` |
| Region address arithmetic | OperationRegion base + field offset | `checked_add`; SystemIO and PCI_Config offsets bounded to 0xFFFF | `fix1_kats.rs` `a_pci_config_field_access_adds_the_region_base` covers the path; the overflow checks are fuzz-covered |
| Results grown from operands | `Concat`, `ConcatRes`, `ToHexString`, `ToDecimalString`, `ToBuffer` of a String | `check_object_size` keeps the result within `MAX_OBJECT_BYTES` (added after the #104 review, item 2e/4a) | `concat_result_over_the_object_limit`, `to_hex_string_result_over_the_object_limit` |
| Field-read allocation | a field's length (a PkgLength, up to 2^28 bits) | a buffer-typed read is capped at `MAX_OBJECT_BYTES` | `field_read_over_the_object_limit` |
| Resource template lengths | descriptor header and length fields | header and `split_at` lengths checked; the address-space minimum length checked | `truncated_resource_descriptors` |
| Recursion through cyclic objects | `Store` of a reference to a Local/Package element | `check_acyclic` refuses a store that would make the value reach the target | `store_that_makes_a_local_refer_to_itself`, `store_that_makes_a_package_contain_itself` |
| Unbounded nesting / growth | blocks, method calls, in-flight operators | `MAX_NESTING` (256) on each | `method_recursion_is_bounded`, `operator_nesting_is_bounded`, `while_without_a_predicate` |

Integer arithmetic on AML Integers (`Add`, `Multiply`, `ShiftLeft`…) is `wrapping_*` in stock already, as AML
defines it, and is unchanged.

**Not covered here:** the slice indexing that remains (for example `bytes[4]` in a resource decoder after its
length check) is not listed line by line. Each instance found by reading or by the fuzz was guarded; an unguarded
one would be a Fix 2 defect.

## Non-termination (OT-86 L7 data, not fixed here)

A fuzz case that runs past the worker's 20 s timeout is a hang. They are not diagnosed one by one here; the
expected causes are AML that loops (a `While` whose predicate never goes false) or waits (`Sleep`, `Stall`,
`Acquire` with a long timeout), which the L7 step budget is meant to bound. They are recorded by input hash so L7 can
replay them (`acpi-aml-fuzz case --seed 0x0786 --index <i>`):

Seed `0x786`, 200000 cases, `fix1`: **17** hangs.

| Case | Input hash | Source |
|---|---|---|
| 1699 | `4b2c37f0b0633c77` | `zenbook-s14-ux5406aa-303/ssdt16.dat@0x6201@0x58` |
| 9471 | `1176ce14ec712d86` | `zenbook-s14-ux5406aa-303/dsdt.dat@0x66ea6@0x0` |
| 17695 | `54a9b9617f7a1ba1` | `optiplex-7080-linux-1.37.0/DSDT@0x6b5e@0x588` |
| 26469 | `d38fbe0e3cab8c9d` | `hp-elitedesk-800-g6-mini-s21-02.27.00/dsdt.dat@0x4617@0x77d9@0x284@0x9ac@0x0` |
| 32642 | `d99e4cf72c359ad9` | `zenbook-s14-ux5406aa-303/dsdt.dat@0x66ff7@0x0` |
| 33425 | `87249ae84a052ba4` | `qemu-q35-ovmf-smp1/dsdt.dat` |
| 64918 | `e9515dd6e768b847` | `hp-elitedesk-800-g6-mini-s21-02.27.00/dsdt.dat@0x4617@0x1d0a@0x43@0x53e` |
| 71629 | `331edd7c03d7f520` | `msi-ms-7e57-1.a70/dsdt.dat@0x3e4c8@0x9@0xda77@0x17da@0x56` |
| 74664 | `0cf931068a193156` | `qemu-q35-ovmf-smp4-viommu/dsdt.dat` |
| 86189 | `c95e9f3e07e92458` | `lenovo-thinkcentre-m910q-m1akt5aa/dsdt.dat@0x28458` |
| 89118 | `7ecacb71c86de8b2` | `zenbook-s14-ux5406aa-303/dsdt.dat@0x66d56@0x0` |
| 94341 | `39a23030bc590311` | `msi-ms-7e57-1.a70/ssdt2.dat@0x2b2c@0x3c3` |
| 117771 | `74fed310f8f4b215` | `qemu-q35-ovmf-smp4-viommu/dsdt.dat@0x5@0x18` |
| 129909 | `50fcc5f2d7451970` | `optiplex-7080-1.37.0/dsdt.dat@0x14839@0x14f` |
| 141514 | `bc1a21128baeb3b3` | `zenbook-s14-ux5406aa-303/dsdt.dat@0x66ff7@0x0` |
| 143561 | `0a0a181f1fff3a3f` | `msi-ms-7e57-1.a70/dsdt.dat@0x3e4c8@0x9@0xda77@0x17da@0x56` |
| 188983 | `33f48df57071b3b6` | `zenbook-s14-ux5406aa-303/ssdt18.dat@0x7e9d@0x26@0xfff@0x73` |

## Out of scope

`src/` outside `src/aml` (`sdt/`, `platform/`, `registers.rs`, `address.rs`, `rsdp.rs`, `lib.rs`) is not part of
Fix 2. Those parse static tables, not AML, and stock panic sites there are unchanged. One path crosses the line:
`Release (\_GL)` writes the PM1 control register through `registers.rs`; the AML side of that call now fails closed
on an error instead of unwrapping (`mod.rs`, `release_global_lock`).
