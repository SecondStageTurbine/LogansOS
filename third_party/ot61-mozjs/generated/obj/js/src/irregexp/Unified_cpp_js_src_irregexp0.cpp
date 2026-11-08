#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/irregexp/RegExpAPI.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/RegExpAPI.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/RegExpAPI.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/RegExpShim.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/RegExpShim.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/RegExpShim.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-bytecode-generator.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-bytecode-generator.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-bytecode-generator.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-bytecode-iterator.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-bytecode-iterator.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-bytecode-iterator.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-code-generator.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-code-generator.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-code-generator.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-dotprinter.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-dotprinter.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-dotprinter.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
