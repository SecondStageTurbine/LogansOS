#define MOZ_UNIFIED_BUILD
#include "@SRC@/memory/mozalloc/mozalloc.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/memory/mozalloc/mozalloc.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/memory/mozalloc/mozalloc.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/memory/mozalloc/mozalloc_abort.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/memory/mozalloc/mozalloc_abort.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/memory/mozalloc/mozalloc_abort.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/memory/mozalloc/mozalloc_oom.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/memory/mozalloc/mozalloc_oom.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/memory/mozalloc/mozalloc_oom.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
