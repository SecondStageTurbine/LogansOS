#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/frontend/CForEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/CForEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/CForEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/CallOrNewEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/CallOrNewEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/CallOrNewEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/DefaultEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/DefaultEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/DefaultEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/DoWhileEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/DoWhileEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/DoWhileEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/ElemOpEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/ElemOpEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/ElemOpEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/EmitterScope.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/EmitterScope.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/EmitterScope.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
