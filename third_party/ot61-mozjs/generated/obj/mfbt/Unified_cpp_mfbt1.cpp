#define MOZ_UNIFIED_BUILD
#include "@SRC@/mfbt/double-conversion/double-conversion/fixed-dtoa.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/fixed-dtoa.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/fixed-dtoa.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/string-to-double.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/string-to-double.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/string-to-double.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/strtod.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/strtod.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/strtod.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
