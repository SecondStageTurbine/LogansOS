/* SPDX-License-Identifier: MPL-2.0 */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/* OT-61 C3c-1: the C++ header of mozjs_normalizer_glue
 * (vendor/mozjs-138c9470/mozjs-extracted-crates/normalizer_glue). Upstream's
 * build.rs writes it with cbindgen; this copy is written by hand from that
 * crate's src/lib.rs and cbindgen.toml (C++, functions and enums only, the
 * four js_call_* / js_new_* callbacks excluded), in cbindgen's layout. The
 * generator will produce it with cbindgen and fail if the two differ. */

#ifndef js_normalizer_glue_h
#define js_normalizer_glue_h

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

enum class NormalizationForm {
  NFC = 0,
  NFKC = 1,
  NFD = 2,
  NFKD = 3,
};

extern "C" {

JSLinearString *js_normalize(JSContext *cx,
                             NormalizationForm form,
                             JSLinearString *in_string,
                             bool latin1);

bool js_normalize_utf16(NormalizationForm form, const uint16_t *ptr, uintptr_t len, void *buffer);

bool js_normalize_latin1(NormalizationForm form, const uint8_t *ptr, uintptr_t len, void *buffer);

uint32_t mozilla_canonical_composition(uint32_t a, uint32_t b);

uint8_t mozilla_canonical_combining_class(uint32_t c);

}  // extern "C"

#endif  // js_normalizer_glue_h
