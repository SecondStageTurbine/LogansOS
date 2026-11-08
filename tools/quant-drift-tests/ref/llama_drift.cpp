// SPDX-License-Identifier: MIT
//! llama.cpp reference runner for the quantization-drift KAT.
//!
//! Prior art: llama.cpp (MIT, pinned commit in `meta.tsv`), public `llama.h`
//! API only. It consumes the SAME prompt token IDs as the Rust harness
//! (`prompts.tsv`) and the same corpus (`corpus.u32`), so no tokenizer is
//! involved here and all three models see identical inputs.
//!
//! Build (from this crate's `ref/` directory):
//!   g++ -O2 -std=c++17 llama_drift.cpp \
//!     -I "$LLAMA_CPP/include" -I "$LLAMA_CPP/ggml/include" \
//!     -L "$LLAMA_CPP/build/bin" -lllama -lggml -lggml-base -lggml-cpu \
//!     -Wl,-rpath,"$LLAMA_CPP/build/bin" -o llama_drift

#include "llama.h"
#include "common.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <string>
#include <vector>

struct Prompt {
    std::string id;
    std::vector<llama_token> ids;
};

static std::vector<Prompt> read_prompts(const std::string & path) {
    std::ifstream in(path);
    if (!in) { fprintf(stderr, "read %s failed\n", path.c_str()); exit(1); }
    std::vector<Prompt> out;
    std::string line;
    while (std::getline(in, line)) {
        if (line.rfind("prompt_ids\t", 0) != 0) continue;
        const size_t tab = line.find('\t', 11);
        if (tab == std::string::npos) continue;
        Prompt prompt;
        prompt.id = line.substr(11, tab - 11);
        std::stringstream tokens(line.substr(tab + 1));
        std::string cell;
        while (std::getline(tokens, cell, ',')) {
            if (!cell.empty()) prompt.ids.push_back(static_cast<llama_token>(std::stoi(cell)));
        }
        out.push_back(std::move(prompt));
    }
    return out;
}

static std::vector<llama_token> read_u32(const std::string & path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) { fprintf(stderr, "read %s failed\n", path.c_str()); exit(1); }
    in.seekg(0, std::ios::end);
    const size_t bytes = static_cast<size_t>(in.tellg());
    in.seekg(0);
    std::vector<uint32_t> words(bytes / 4);
    in.read(reinterpret_cast<char *>(words.data()), static_cast<std::streamsize>(words.size() * 4));
    std::vector<llama_token> out(words.size());
    for (size_t i = 0; i < words.size(); ++i) out[i] = static_cast<llama_token>(words[i]);
    return out;
}

static int argmax(const float * logits, int n_vocab) {
    int best = 0;
    float best_value = logits[0];
    for (int i = 1; i < n_vocab; ++i) {
        if (logits[i] > best_value) { best_value = logits[i]; best = i; }
    }
    return best;
}

static double nll_at(const float * logits, int n_vocab, llama_token target) {
    float max_logit = logits[0];
    for (int i = 1; i < n_vocab; ++i) max_logit = std::max(max_logit, logits[i]);
    double sum = 0.0;
    for (int i = 0; i < n_vocab; ++i) sum += std::exp(static_cast<double>(logits[i] - max_logit));
    const double lse = static_cast<double>(max_logit) + std::log(sum);
    return lse - static_cast<double>(logits[target]);
}

static void clear_context(llama_context * ctx) {
    llama_memory_t mem = llama_get_memory(ctx);
    if (mem) llama_memory_clear(mem, true);
}

// Feed `tokens[begin..end)` at positions begin.., all outputs requested.
// Uses llama.cpp's current `common_batch` + `llama_process` API (the one
// llama-perplexity uses); the legacy `llama_batch`/`llama_decode` path does
// not compute every requested output row on this build.
static bool decode_range(llama_context * ctx, const std::vector<llama_token> & tokens,
                         size_t begin, size_t end, int n_batch) {
    for (size_t offset = begin; offset < end; offset += static_cast<size_t>(n_batch)) {
        const size_t count = std::min(static_cast<size_t>(n_batch), end - offset);
        common_batch batch(ctx);
        for (size_t i = 0; i < count; ++i) {
            batch.add(tokens[offset + i], static_cast<llama_pos>(offset + i), 0, true);
        }
        if (llama_process(ctx, LLAMA_PROCESS_TYPE_DECODE, batch.get()) != 0) {
            return false;
        }
    }
    return true;
}

int main(int argc, char ** argv) {
    if (argc < 7) {
        fprintf(stderr,
            "usage: %s MODEL PROMPTS_TSV CORPUS_U32 OUT_TSV LABEL SOURCE [SHA256] [TOOL]\n"
            "       [--eval-start N] [--corpus-tokens N] [--corpus-blake3 HASH]\n", argv[0]);
        return 2;
    }
    const std::string model_path = argv[1];
    const std::string prompts_path = argv[2];
    const std::string corpus_path = argv[3];
    const std::string out_path = argv[4];
    const std::string label = argv[5];
    const std::string source = argv[6];
    std::string sha256 = argc > 7 ? argv[7] : "";
    std::string tool = argc > 8 ? argv[8] : "";
    int eval_start = 4096;
    int corpus_tokens = 8192;
    std::string corpus_blake3;
    std::string only_prompt;
    bool one_by_one = false;
    bool ppl_one_by_one = false;
    for (int i = 9; i + 1 < argc; i += 2) {
        if (std::strcmp(argv[i], "--eval-start") == 0) eval_start = std::atoi(argv[i + 1]);
        else if (std::strcmp(argv[i], "--corpus-tokens") == 0) corpus_tokens = std::atoi(argv[i + 1]);
        else if (std::strcmp(argv[i], "--corpus-blake3") == 0) corpus_blake3 = argv[i + 1];
        else if (std::strcmp(argv[i], "--only") == 0) only_prompt = argv[i + 1];
        else if (std::strcmp(argv[i], "--one-by-one") == 0) one_by_one = std::atoi(argv[i + 1]) != 0;
        else if (std::strcmp(argv[i], "--ppl-one-by-one") == 0) ppl_one_by_one = std::atoi(argv[i + 1]) != 0;
    }

    const int n_threads = 4;
    llama_backend_init();
    llama_model_params mparams = llama_model_default_params();
    mparams.n_gpu_layers = 0;
    llama_model * model = llama_model_load_from_file(model_path.c_str(), mparams);
    if (!model) { fprintf(stderr, "load model failed\n"); return 1; }
    const int n_vocab = llama_vocab_n_tokens(llama_model_get_vocab(model));

    llama_context_params cparams = llama_context_default_params();
    cparams.n_ctx = static_cast<uint32_t>(corpus_tokens + 64);
    cparams.n_batch = 512;
    cparams.n_ubatch = 512;
    cparams.n_threads = n_threads;
    cparams.n_threads_batch = n_threads;
    cparams.n_outputs_max = 512;
    cparams.n_outputs_max_per_seq = 512;
    cparams.no_perf = true;

    std::vector<Prompt> prompts = read_prompts(prompts_path);
    std::vector<llama_token> corpus = read_u32(corpus_path);
    if (static_cast<int>(corpus.size()) < corpus_tokens) {
        fprintf(stderr, "corpus has %zu tokens, need %d\n", corpus.size(), corpus_tokens);
        return 1;
    }
    corpus.resize(corpus_tokens);

    std::ostringstream out;
    out << "schema\tquant-drift-ref-v1\n";
    out << "model\t" << label << "\n";
    out << "model_sha256\t" << sha256 << "\n";
    out << "model_source\t" << source << "\n";
    out << "tool\t" << tool << "\n";
    out << "corpus_tokens\t" << corpus_tokens << "\n";
    out << "corpus_eval_start\t" << eval_start << "\n";
    out << "corpus_slice_blake3\t" << corpus_blake3 << "\n";
    out << "prompts\t" << prompts.size() << "\n";

    llama_context * ctx = llama_init_from_model(model, cparams);
    if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }

    for (const Prompt & prompt : prompts) {
        if (!only_prompt.empty() && prompt.id != only_prompt) continue;
        // A fresh context per prompt: `llama_memory_clear` leaves the output
        // mapping stale across a shorter prompt, which made every non-last
        // position read a previous decode's row.
        llama_free(ctx);
        ctx = llama_init_from_model(model, cparams);
        if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }
        if (prompt.ids.empty() || prompt.ids.size() + 64 > cparams.n_ctx) {
            fprintf(stderr, "prompt %s does not fit\n", prompt.id.c_str());
            return 1;
        }
        clear_context(ctx);
        if (!decode_range(ctx, prompt.ids, 0, prompt.ids.size(), 512)) {
            fprintf(stderr, "decode prompt %s failed\n", prompt.id.c_str());
            return 1;
        }
        std::vector<llama_token> top1;
        top1.reserve(prompt.ids.size());
        const float * last_logits = nullptr;
        const float * all_logits = llama_get_logits(ctx);
        if (!all_logits) { fprintf(stderr, "no prompt logits\n"); return 1; }
        for (size_t i = 0; i < prompt.ids.size(); ++i) {
            const float * logits = all_logits + i * static_cast<size_t>(n_vocab);
            if (!logits) { fprintf(stderr, "no logits at %zu\n", i); return 1; }
            top1.push_back(static_cast<llama_token>(argmax(logits, n_vocab)));
            if (i + 1 == prompt.ids.size()) last_logits = logits;
        }
        if (!only_prompt.empty() && prompt.ids.size() <= 8) {
            const float * all = llama_get_logits(ctx);
            for (size_t i = 0; i < prompt.ids.size(); ++i) {
                const float * row = all + i * static_cast<size_t>(n_vocab);
                int b0=-1,b1=-1,b2=-1; float v0=-1e30f,v1=-1e30f,v2=-1e30f;
                for (int t=0;t<n_vocab;++t) {
                    if (row[t]>v0) { v2=v1;b2=b1; v1=v0;b1=b0; v0=row[t];b0=t; }
                    else if (row[t]>v1) { v2=v1;b2=b1; v1=row[t];b1=t; }
                    else if (row[t]>v2) { v2=row[t];b2=t; }
                }
                fprintf(stderr, "ROW %zu top=[%d(%.3f) %d(%.3f) %d(%.3f)] want_next=%d\n",
                        i,b0,v0,b1,v1,b2,v2, i+1<prompt.ids.size()?prompt.ids[i+1]:-1);
            }
        }
        if (one_by_one) {
            llama_free(ctx);
            ctx = llama_init_from_model(model, cparams);
            if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }
            top1.clear();
            std::vector<llama_token> history;
            for (size_t i = 0; i < prompt.ids.size(); ++i) {
                history.push_back(prompt.ids[i]);
                decode_range(ctx, history, i, i + 1, 1);
                const float * logits = llama_get_logits(ctx);
                top1.push_back(static_cast<llama_token>(argmax(logits, n_vocab)));
                if (i + 1 == prompt.ids.size()) last_logits = logits;
            }
        }

        std::vector<llama_token> generated;
        llama_token next = static_cast<llama_token>(argmax(last_logits, n_vocab));
        generated.push_back(next);
        for (int step = 1; step < 32; ++step) {
            const size_t position = prompt.ids.size() + static_cast<size_t>(step) - 1;
            common_batch batch(ctx);
            batch.add(next, static_cast<llama_pos>(position), 0, true);
            if (llama_process(ctx, LLAMA_PROCESS_TYPE_DECODE, batch.get()) != 0) {
                fprintf(stderr, "decode step failed\n");
                return 1;
            }
            const float * logits = llama_get_logits(ctx);
            if (!logits) { fprintf(stderr, "no logits at step %d\n", step); return 1; }
            next = static_cast<llama_token>(argmax(logits, n_vocab));
            generated.push_back(next);
        }

        out << "prompt\t" << prompt.id << "\n";
        out << "prompt_ids\t" << prompt.id << "\t";
        for (size_t i = 0; i < prompt.ids.size(); ++i) out << (i ? "," : "") << prompt.ids[i];
        out << "\n";
        out << "generated\t" << prompt.id << "\t";
        for (size_t i = 0; i < generated.size(); ++i) out << (i ? "," : "") << generated[i];
        out << "\n";
        out << "top1\t" << prompt.id << "\t";
        for (size_t i = 0; i < top1.size(); ++i) out << (i ? "," : "") << top1[i];
        out << "\n";
        fprintf(stderr, "prompt %s: %zu tokens, generated first=%d\n",
                prompt.id.c_str(), prompt.ids.size(), generated.front());
    }

    // Corpus perplexity: NLL of token i+1 from the logits at position i, for
    // i in [eval_start, len - 1) — 4,095 targets by default.
    if (ppl_one_by_one) {
        // One token per decode: the multi-output prompt path on this llama.cpp
        // build returns stale rows for non-final positions, but a single-token
        // decode's last (only) row is the one generation already trusts.
        llama_free(ctx);
        ctx = llama_init_from_model(model, cparams);
        if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }
        double nll = 0.0;
        size_t count = 0;
        for (size_t i = 0; i + 1 < corpus.size(); ++i) {
            common_batch batch(ctx);
            batch.add(corpus[i], static_cast<llama_pos>(i), 0, true);
            if (llama_process(ctx, LLAMA_PROCESS_TYPE_DECODE, batch.get()) != 0) {
                fprintf(stderr, "corpus decode failed at %zu\n", i);
                return 1;
            }
            if (static_cast<int>(i) < eval_start) continue;
            const float * logits = llama_get_logits(ctx);
            nll += nll_at(logits, n_vocab, corpus[i + 1]);
            ++count;
            if (i % 512 == 0) fprintf(stderr, "corpus %zu/%zu\r", i, corpus.size());
        }
        fprintf(stderr, "\n");
        out << "corpus_nll_sum\t" << std::fixed << std::setprecision(17) << nll << "\n";
        out << "corpus_nll_count\t" << count << "\n";
        std::ofstream file(out_path);
        if (!file) { fprintf(stderr, "write %s failed\n", out_path.c_str()); return 1; }
        file << out.str();
        fprintf(stderr, "%s: PPL %.6f over %zu targets\n", label.c_str(),
                std::exp(nll / static_cast<double>(count)), count);
        llama_free(ctx);
        llama_model_free(model);
        llama_backend_free();
        return 0;
    }
    clear_context(ctx);
    double nll_sum = 0.0;
    size_t nll_count = 0;
    const size_t chunk = 512;
    for (size_t begin = 0; begin < corpus.size(); begin += chunk) {
        const size_t end = std::min(begin + chunk, corpus.size());
        if (!decode_range(ctx, corpus, begin, end, 512)) {
            fprintf(stderr, "corpus decode failed at %zu\n", begin);
            return 1;
        }
        for (size_t i = begin; i < end; ++i) {
            if (static_cast<int>(i) < eval_start || i + 1 >= corpus.size()) continue;
            const float * logits = llama_get_logits(ctx) + (i - begin) * static_cast<size_t>(n_vocab);
            if (!logits) { fprintf(stderr, "no corpus logits at %zu\n", i); return 1; }
            nll_sum += nll_at(logits, n_vocab, corpus[i + 1]);
            ++nll_count;
        }
        fprintf(stderr, "corpus %zu/%zu\r", end, corpus.size());
    }
    fprintf(stderr, "\n");
    out << "corpus_nll_sum\t" << std::fixed << std::setprecision(17) << nll_sum << "\n";
    out << "corpus_nll_count\t" << nll_count << "\n";

    std::ofstream file(out_path);
    if (!file) { fprintf(stderr, "write %s failed\n", out_path.c_str()); return 1; }
    file << out.str();
    fprintf(stderr, "%s: PPL %.6f over %zu targets\n", label.c_str(),
            std::exp(nll_sum / static_cast<double>(nll_count)), nll_count);

    llama_free(ctx);
    llama_model_free(model);
    llama_backend_free();
    return 0;
}
