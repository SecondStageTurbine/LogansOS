#define MOZ_UNIFIED_BUILD
#include "@SRC@/js/src/frontend/NameOpEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/NameOpEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/NameOpEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/ObjLiteral.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/ObjLiteral.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/ObjLiteral.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/ObjectEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/ObjectEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/ObjectEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/OptionalEmitter.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/OptionalEmitter.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/OptionalEmitter.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/ParseContext.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/ParseContext.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/ParseContext.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/js/src/frontend/ParseNode.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/js/src/frontend/ParseNode.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/js/src/frontend/ParseNode.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
