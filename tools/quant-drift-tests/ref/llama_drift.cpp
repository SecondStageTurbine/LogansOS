// SPDX-License-Identifier: MIT
//! llama.cpp reference runner for the quantization-drift KAT.
//!
//! Prior art: llama.cpp (MIT, pinned commit in `meta.tsv`), public `llama.h`
//! API only. It consumes the SAME prompt token IDs as the Rust harness
//! (`prompts.tsv`) and the same corpus (`corpus.u32`).
//!
//! Per-position logits come from a real `llama_batch` with `logits[i] = 1` for
//! EVERY token, decoded through `llama_decode()`; rows are read with
//! `llama_get_logits_ith(ctx, i)`.  `--verify-final N` re-feeds each of the
//! first N positions in a fresh context and compares the last-position row to
//! the multi-output row, which is the correctness check for the batch path.

#include "llama.h"
#include "ggml.h"
#include "llama-ext.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <map>
#include <sstream>
#include <string>
#include <vector>

struct Prompt {
    std::string id;
    std::string text_hex;
    std::vector<llama_token> ids;
};

static std::vector<Prompt> read_prompts(const std::string & path) {
    std::ifstream in(path);
    if (!in) { fprintf(stderr, "read %s failed\n", path.c_str()); exit(1); }
    std::vector<Prompt> out;
    std::string line;
    while (std::getline(in, line)) {
        if (line.rfind("prompt\t", 0) == 0) {
            const size_t a = line.find('\t', 7);
            if (a == std::string::npos) continue;
            const std::string id = line.substr(7, a - 7);
            for (auto & p : out) if (p.id == id) p.text_hex = line.substr(a + 1);
            continue;
        }
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

struct Top2 {
    int id = -1;
    int second = -1;
    float v1 = 0.0f;
    float v2 = 0.0f;
};

static Top2 top2_of(const float * row, int n_vocab) {
    Top2 t;
    t.id = 0;
    t.second = 1;
    if (row[t.second] > row[t.id]) std::swap(t.id, t.second);
    for (int i = 2; i < n_vocab; ++i) {
        if (row[i] > row[t.id]) { t.second = t.id; t.id = i; }
        else if (row[i] > row[t.second]) { t.second = i; }
    }
    t.v1 = row[t.id];
    t.v2 = row[t.second];
    return t;
}

static double nll_at(const float * logits, int n_vocab, llama_token target) {
    float max_logit = logits[0];
    for (int i = 1; i < n_vocab; ++i) max_logit = std::max(max_logit, logits[i]);
    double sum = 0.0;
    for (int i = 0; i < n_vocab; ++i) sum += std::exp(static_cast<double>(logits[i] - max_logit));
    const double lse = static_cast<double>(max_logit) + std::log(sum);
    return lse - static_cast<double>(logits[target]);
}

// Feed `tokens[begin..end)` at positions begin.., request every output row.
static bool decode_range(llama_context * ctx, const std::vector<llama_token> & tokens,
                         size_t begin, size_t end, int n_batch) {
    for (size_t offset = begin; offset < end; offset += static_cast<size_t>(n_batch)) {
        const int count = static_cast<int>(std::min(static_cast<size_t>(n_batch), end - offset));
        llama_batch batch = llama_batch_init(count, 0, 1);
        for (int i = 0; i < count; ++i) {
            batch.token[i] = tokens[offset + static_cast<size_t>(i)];
            batch.pos[i] = static_cast<llama_pos>(offset + static_cast<size_t>(i));
            batch.n_seq_id[i] = 1;
            batch.seq_id[i][0] = 0;
            batch.logits[i] = 1;
        }
        batch.n_tokens = count;
        const int rc = llama_decode(ctx, batch);
        llama_batch_free(batch);
        if (rc != 0) {
            fprintf(stderr, "llama_decode failed at %zu (rc=%d)\n", offset, rc);
            return false;
        }
    }
    return true;
}

static std::string join_ids(const std::vector<llama_token> & ids) {
    std::ostringstream out;
    for (size_t i = 0; i < ids.size(); ++i) out << (i ? "," : "") << ids[i];
    return out.str();
}

static std::string join_floats(const std::vector<float> & values) {
    std::ostringstream out;
    out << std::fixed << std::setprecision(6);
    for (size_t i = 0; i < values.size(); ++i) out << (i ? "," : "") << values[i];
    return out.str();
}

static void append_top2(std::ostringstream & out, const std::string & name, const std::string & id,
                        const std::vector<int> & top1, const std::vector<int> & top2,
                        const std::vector<float> & v1, const std::vector<float> & v2) {
    out << name << "\t" << id << "\t" << join_ids(std::vector<llama_token>(top1.begin(), top1.end())) << "\n";
    out << name << "_2nd\t" << id << "\t" << join_ids(std::vector<llama_token>(top2.begin(), top2.end())) << "\n";
    out << name << "_logit\t" << id << "\t" << join_floats(v1) << "\n";
    out << name << "_2nd_logit\t" << id << "\t" << join_floats(v2) << "\n";
}

static std::map<std::string, std::vector<int>> read_positions(const std::string & path) {
    std::map<std::string, std::vector<int>> out;
    std::ifstream in(path);
    if (!in) { fprintf(stderr, "read %s failed\n", path.c_str()); exit(1); }
    std::string line;
    while (std::getline(in, line)) {
        if (line.empty() || line[0] == '#') continue;
        const size_t tab = line.find('\t');
        if (tab == std::string::npos) {
            fprintf(stderr, "%s: expected <prompt>\\t<index>, got %s\n", path.c_str(), line.c_str());
            exit(1);
        }
        const std::string id = line.substr(0, tab);
        out[id].push_back(std::atoi(line.c_str() + tab + 1));
    }
    for (auto & entry : out) std::sort(entry.second.begin(), entry.second.end());
    return out;
}

static void write_u32(std::ofstream & out, uint32_t value) {
    out.write(reinterpret_cast<const char *>(&value), sizeof(value));
}

// Same `QDLG` format as `quant-drift-tests logits`: magic, version, n_vocab,
// row_count, then per row: u32 id length, id bytes, u32 position, f32 logits.
static void write_logit_row(std::ofstream & out, const std::string & id, int index,
                            const float * row, int n_vocab) {
    write_u32(out, static_cast<uint32_t>(id.size()));
    out.write(id.data(), static_cast<std::streamsize>(id.size()));
    write_u32(out, static_cast<uint32_t>(index));
    out.write(reinterpret_cast<const char *>(row), static_cast<std::streamsize>(n_vocab) * 4);
}

static void write_layer_header(std::ofstream & out, uint32_t n_embd, uint32_t n_layers, uint32_t rows) {
    out.write("QDLH", 4);
    write_u32(out, 1);
    write_u32(out, n_embd);
    write_u32(out, n_layers);
    write_u32(out, rows);
}

static void write_layer_row(std::ofstream & out, const std::string & id, int index,
                            const std::vector<std::vector<float>> & layers) {
    write_u32(out, static_cast<uint32_t>(id.size()));
    out.write(id.data(), static_cast<std::streamsize>(id.size()));
    write_u32(out, static_cast<uint32_t>(index));
    for (const auto & layer : layers) {
        out.write(reinterpret_cast<const char *>(layer.data()),
                  static_cast<std::streamsize>(layer.size()) * 4);
    }
}

int main(int argc, char ** argv) {
    if (argc < 7) {
        fprintf(stderr,
            "usage: %s MODEL PROMPTS_TSV CORPUS_U32 OUT_TSV LABEL SOURCE [SHA256] [TOOL]\n"
            "       [--eval-start N] [--corpus-tokens N] [--corpus-blake3 HASH]\n"
            "       [--verify-final N] [--no-generate 1]\n", argv[0]);
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
    int verify_final = 0;
    bool generate = true;
    std::string corpus_blake3;
    std::string dump_positions_path;
    std::string dump_logits_path;
    std::string dump_layers_path;
    bool kv_f32 = false;
    for (int i = 9; i + 1 < argc; i += 2) {
        if (std::strcmp(argv[i], "--eval-start") == 0) eval_start = std::atoi(argv[i + 1]);
        else if (std::strcmp(argv[i], "--corpus-tokens") == 0) corpus_tokens = std::atoi(argv[i + 1]);
        else if (std::strcmp(argv[i], "--corpus-blake3") == 0) corpus_blake3 = argv[i + 1];
        else if (std::strcmp(argv[i], "--verify-final") == 0) verify_final = std::atoi(argv[i + 1]);
        else if (std::strcmp(argv[i], "--no-generate") == 0) generate = std::atoi(argv[i + 1]) == 0;
        else if (std::strcmp(argv[i], "--dump-positions") == 0) dump_positions_path = argv[i + 1];
        else if (std::strcmp(argv[i], "--dump-logits") == 0) dump_logits_path = argv[i + 1];
        else if (std::strcmp(argv[i], "--kv-f32") == 0) kv_f32 = std::atoi(argv[i + 1]) != 0;
        else if (std::strcmp(argv[i], "--dump-layer-inputs") == 0) dump_layers_path = argv[i + 1];
    }
    if (dump_logits_path.empty() != dump_positions_path.empty()) {
        fprintf(stderr, "--dump-logits and --dump-positions must be given together\n");
        return 2;
    }
    const std::map<std::string, std::vector<int>> dump_positions =
        dump_positions_path.empty() ? std::map<std::string, std::vector<int>>()
                                    : read_positions(dump_positions_path);
    std::ofstream dump_logits;
    if (!dump_logits_path.empty()) {
        dump_logits.open(dump_logits_path, std::ios::binary);
        if (!dump_logits) { fprintf(stderr, "write %s failed\n", dump_logits_path.c_str()); return 1; }
    }
    std::ofstream dump_layers;
    if (!dump_layers_path.empty()) {
        dump_layers.open(dump_layers_path, std::ios::binary);
        if (!dump_layers) { fprintf(stderr, "write %s failed\n", dump_layers_path.c_str()); return 1; }
    }

    const int n_threads = 4;
    llama_backend_init();
    llama_model_params mparams = llama_model_default_params();
    mparams.n_gpu_layers = 0;
    llama_model * model = llama_model_load_from_file(model_path.c_str(), mparams);
    if (!model) { fprintf(stderr, "load model failed\n"); return 1; }
    const int n_vocab = llama_vocab_n_tokens(llama_model_get_vocab(model));
    const int n_embd = llama_model_n_embd(model);
    const int n_layer = llama_model_n_layer(model);
    if (dump_layers.is_open()) {
        uint32_t total = 0;
        for (const auto & entry : dump_positions) total += static_cast<uint32_t>(entry.second.size());
        write_layer_header(dump_layers, static_cast<uint32_t>(n_embd), static_cast<uint32_t>(n_layer), total);
    }
    if (dump_logits.is_open()) {
        uint32_t total = 0;
        for (const auto & entry : dump_positions) total += static_cast<uint32_t>(entry.second.size());
        dump_logits.write("QDLG", 4);
        write_u32(dump_logits, 1);
        write_u32(dump_logits, static_cast<uint32_t>(n_vocab));
        write_u32(dump_logits, total);
    }

    llama_context_params cparams = llama_context_default_params();
    cparams.n_ctx = static_cast<uint32_t>(corpus_tokens + 64);
    cparams.n_batch = 512;
    cparams.n_ubatch = 512;
    cparams.n_threads = n_threads;
    cparams.n_threads_batch = n_threads;
    cparams.n_outputs_max = 512;
    cparams.n_outputs_max_per_seq = 512;
    cparams.no_perf = true;
    if (kv_f32) {
        cparams.type_k = GGML_TYPE_F32;
        cparams.type_v = GGML_TYPE_F32;
    }

    std::vector<Prompt> prompts = read_prompts(prompts_path);
    std::vector<llama_token> corpus = read_u32(corpus_path);
    if (static_cast<int>(corpus.size()) < corpus_tokens) {
        fprintf(stderr, "corpus has %zu tokens, need %d\n", corpus.size(), corpus_tokens);
        return 1;
    }
    corpus.resize(corpus_tokens);

    std::ostringstream out;
    out << "schema\tquant-drift-ref-v2\n";
    out << "model\t" << label << "\n";
    out << "model_sha256\t" << sha256 << "\n";
    out << "model_source\t" << source << "\n";
    out << "tool\t" << tool << "\n";
    out << "corpus_tokens\t" << corpus_tokens << "\n";
    out << "corpus_eval_start\t" << eval_start << "\n";
    out << "corpus_slice_blake3\t" << corpus_blake3 << "\n";
    out << "prompts\t" << prompts.size() << "\n";

    int verified = 0;
    double max_final_delta = 0.0;
    for (const Prompt & prompt : prompts) {
        if (prompt.ids.empty()) { fprintf(stderr, "empty prompt %s\n", prompt.id.c_str()); return 1; }
        llama_context * ctx = llama_init_from_model(model, cparams);
        if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }
        if (dump_layers.is_open()) {
            for (int lid = 0; lid < n_layer; ++lid) llama_set_embeddings_layer_inp(ctx, static_cast<uint32_t>(lid), true);
        }
        if (!decode_range(ctx, prompt.ids, 0, prompt.ids.size(), 512)) return 1;
        std::vector<int> top1, top2;
        std::vector<float> v1, v2;
        top1.reserve(prompt.ids.size());
        top2.reserve(prompt.ids.size());
        v1.reserve(prompt.ids.size());
        v2.reserve(prompt.ids.size());
        for (size_t i = 0; i < prompt.ids.size(); ++i) {
            const float * row = llama_get_logits_ith(ctx, static_cast<int32_t>(i));
            if (!row) { fprintf(stderr, "no logits at %zu\n", i); return 1; }
            const Top2 t = top2_of(row, n_vocab);
            top1.push_back(t.id);
            top2.push_back(t.second);
            v1.push_back(t.v1);
            v2.push_back(t.v2);
        }
        const auto wanted = dump_positions.find(prompt.id);
        if (dump_layers.is_open() && wanted != dump_positions.end()) {
            for (const int index : wanted->second) {
                std::vector<std::vector<float>> layers;
                layers.reserve(static_cast<size_t>(n_layer));
                for (int lid = 0; lid < n_layer; ++lid) {
                    const float * row = llama_get_embeddings_layer_inp(ctx, static_cast<uint32_t>(lid)) + static_cast<size_t>(index) * n_embd;
                    layers.emplace_back(row, row + n_embd);
                }
                write_layer_row(dump_layers, prompt.id, index, layers);
            }
        }
        if (dump_logits.is_open() && wanted != dump_positions.end()) {
            for (const int index : wanted->second) {
                if (index < 0 || static_cast<size_t>(index) >= prompt.ids.size()) {
                    fprintf(stderr, "%s: position %d out of range\n", prompt.id.c_str(), index);
                    return 1;
                }
                const float * row = llama_get_logits_ith(ctx, index);
                if (!row) { fprintf(stderr, "no logits at %d\n", index); return 1; }
                write_logit_row(dump_logits, prompt.id, index, row, n_vocab);
            }
        }
        const float * last_logits = llama_get_logits_ith(ctx, static_cast<int32_t>(prompt.ids.size() - 1));
        std::vector<llama_token> generated;
        std::vector<int> generated_top2;
        std::vector<float> generated_logits;
        std::vector<float> generated_top2_logits;
        if (generate) {
            Top2 choice = top2_of(last_logits, n_vocab);
            generated.push_back(static_cast<llama_token>(choice.id));
            generated_top2.push_back(choice.second);
            generated_logits.push_back(choice.v1);
            generated_top2_logits.push_back(choice.v2);
            for (int step = 1; step < 32; ++step) {
                const size_t position = prompt.ids.size() + static_cast<size_t>(step) - 1;
                llama_batch batch = llama_batch_init(1, 0, 1);
                batch.token[0] = generated.back();
                batch.pos[0] = static_cast<llama_pos>(position);
                batch.n_seq_id[0] = 1;
                batch.seq_id[0][0] = 0;
                batch.logits[0] = 1;
                batch.n_tokens = 1;
                const int rc = llama_decode(ctx, batch);
                llama_batch_free(batch);
                if (rc != 0) { fprintf(stderr, "decode step failed\n"); return 1; }
                const float * row = llama_get_logits_ith(ctx, 0);
                choice = top2_of(row, n_vocab);
                generated.push_back(static_cast<llama_token>(choice.id));
                generated_top2.push_back(choice.second);
                generated_logits.push_back(choice.v1);
                generated_top2_logits.push_back(choice.v2);
            }
        }

        // Correctness check: the multi-output row at position i must agree with
        // the last-position row of a fresh context fed tokens[0..=i].
        if (verify_final > 0) {
            const size_t count = std::min(static_cast<size_t>(verify_final), prompt.ids.size());
            for (size_t i = 0; i < count; ++i) {
                llama_context * vctx = llama_init_from_model(model, cparams);
                if (!vctx) { fprintf(stderr, "verify context init failed\n"); return 1; }
                if (!decode_range(vctx, prompt.ids, 0, i + 1, 512)) return 1;
                const float * final_row = llama_get_logits_ith(vctx, static_cast<int32_t>(i));
                if (!final_row) { fprintf(stderr, "no final logits at %zu\n", i); return 1; }
                const Top2 t = top2_of(final_row, n_vocab);
                if (t.id != top1[i]) {
                    fprintf(stderr, "VERIFY FINAL mismatch %s pos %zu batch=%d final=%d\n",
                            prompt.id.c_str(), i, top1[i], t.id);
                    return 1;
                }
                max_final_delta = std::max<double>(max_final_delta, std::fabs(t.v1 - v1[i]));
                llama_free(vctx);
                ++verified;
            }
        }
        llama_free(ctx);

        out << "prompt\t" << prompt.id << "\t" << prompt.text_hex << "\n";
        out << "prompt_ids\t" << prompt.id << "\t" << join_ids(prompt.ids) << "\n";
        out << "generated\t" << prompt.id << "\t" << join_ids(generated) << "\n";
        if (!generated.empty()) {
            out << "generated_2nd\t" << prompt.id << "\t"
                << join_ids(std::vector<llama_token>(generated_top2.begin(), generated_top2.end())) << "\n";
            out << "generated_logit\t" << prompt.id << "\t" << join_floats(generated_logits) << "\n";
            out << "generated_2nd_logit\t" << prompt.id << "\t" << join_floats(generated_top2_logits) << "\n";
        }
        append_top2(out, "top1", prompt.id, top1, top2, v1, v2);
        fprintf(stderr, "prompt %s: %zu tokens, generated first=%d\n",
                prompt.id.c_str(), prompt.ids.size(), generated.empty() ? -1 : generated.front());
    }
    if (verify_final > 0) {
        fprintf(stderr, "verify-final: %d positions, max |batch - final| top-1 logit = %.6f\n",
                verified, max_final_delta);
        out << "verify_final_positions\t" << verified << "\n";
        out << "verify_final_max_logit_delta\t" << std::fixed << std::setprecision(9)
            << max_final_delta << "\n";
    }

    // Corpus perplexity: NLL of token i+1 from the logits at position i, for
    // i in [eval_start, len - 1).
    llama_context * ctx = llama_init_from_model(model, cparams);
    if (!ctx) { fprintf(stderr, "context init failed\n"); return 1; }
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
            const float * logits = llama_get_logits_ith(ctx, static_cast<int32_t>(i - begin));
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
