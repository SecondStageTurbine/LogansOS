#define MOZ_UNIFIED_BUILD
#include "@SRC@/mfbt/Assertions.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/Assertions.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/Assertions.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/ChaosMode.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/ChaosMode.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/ChaosMode.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/FloatingPoint.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/FloatingPoint.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/FloatingPoint.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/HashFunctions.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/HashFunctions.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/HashFunctions.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/Poison.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/Poison.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/Poison.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/RandomNum.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/RandomNum.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/RandomNum.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/RefCounted.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/RefCounted.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/RefCounted.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/SHA1.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/SHA1.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/SHA1.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/TaggedAnonymousMemory.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/TaggedAnonymousMemory.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/TaggedAnonymousMemory.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/UniquePtrExtensions.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/UniquePtrExtensions.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/UniquePtrExtensions.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/Utf8.cpp"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/Utf8.cpp uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/Utf8.cpp defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/bignum-dtoa.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/bignum-dtoa.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/bignum-dtoa.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/bignum.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/bignum.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/bignum.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/cached-powers.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/cached-powers.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/cached-powers.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/double-to-string.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/double-to-string.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/double-to-string.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
#include "@SRC@/mfbt/double-conversion/double-conversion/fast-dtoa.cc"
#ifdef PL_ARENA_CONST_ALIGN_MASK
#error "@SRC@/mfbt/double-conversion/double-conversion/fast-dtoa.cc uses PL_ARENA_CONST_ALIGN_MASK, so it cannot be built in unified mode."
#undef PL_ARENA_CONST_ALIGN_MASK
#endif
#ifdef INITGUID
#error "@SRC@/mfbt/double-conversion/double-conversion/fast-dtoa.cc defines INITGUID, so it cannot be built in unified mode."
#undef INITGUID
#endif
