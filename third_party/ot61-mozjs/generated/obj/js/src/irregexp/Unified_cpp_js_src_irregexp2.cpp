#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/irregexp/util/UnicodeShim.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/irregexp/util/UnicodeShim.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/irregexp/util/UnicodeShim.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
