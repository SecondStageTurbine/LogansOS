#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/wasm/WasmOpIter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmOpIter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmOpIter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/wasm/WasmPI.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmPI.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmPI.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/wasm/WasmProcess.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmProcess.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmProcess.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/wasm/WasmRealm.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmRealm.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmRealm.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/wasm/WasmSerialize.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmSerialize.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmSerialize.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/wasm/WasmSignalHandlers.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/wasm/WasmSignalHandlers.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/wasm/WasmSignalHandlers.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
