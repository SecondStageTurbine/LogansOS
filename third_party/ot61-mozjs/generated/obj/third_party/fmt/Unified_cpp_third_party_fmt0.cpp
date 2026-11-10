#define MOZ_UNIFIED_BUILD
#include "@SRC@/third_party/fmt/src/format.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/third_party/fmt/src/format.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/third_party/fmt/src/format.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
