#define MOZ_UNIFIED_BUILD
#include "@SRC@/mozglue/baseprofiler/core/BaseAndGeckoProfilerDetail.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mozglue/baseprofiler/core/BaseAndGeckoProfilerDetail.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mozglue/baseprofiler/core/BaseAndGeckoProfilerDetail.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mozglue/baseprofiler/core/Flow.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mozglue/baseprofiler/core/Flow.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mozglue/baseprofiler/core/Flow.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mozglue/baseprofiler/core/ProfilerUtils.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mozglue/baseprofiler/core/ProfilerUtils.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mozglue/baseprofiler/core/ProfilerUtils.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
