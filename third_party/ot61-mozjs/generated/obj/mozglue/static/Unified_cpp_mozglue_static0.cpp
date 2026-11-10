#define MOZ_UNIFIED_BUILD
#include "@SRC@/mozglue/static/Compression.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mozglue/static/Compression.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mozglue/static/Compression.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
