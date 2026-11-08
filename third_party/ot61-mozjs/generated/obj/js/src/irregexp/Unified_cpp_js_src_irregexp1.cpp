#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/irregexp/imported/regexp-interpreter.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-interpreter.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-interpreter.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-macro-assembler-tracer.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-macro-assembler-tracer.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-macro-assembler-tracer.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-macro-assembler.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-macro-assembler.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-macro-assembler.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-parser.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-parser.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-parser.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-printer.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-printer.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-printer.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/irregexp/imported/regexp-stack.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/imported/regexp-stack.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/imported/regexp-stack.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
