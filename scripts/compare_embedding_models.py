from pathlib import Path
import json
import os
import sys

from sentence_transformers import SentenceTransformer

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "relevance" / "long_horizon_cases.tsv"


def load_facts():
    rows = []
    lines = FIXTURE.read_text(encoding="utf-8").splitlines()
    for line in lines[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        assert len(parts) == 5, (len(parts), parts)
        rows.append((parts[0], parts[1], parts[2:]))
    return rows


def cosine(a, b):
    return sum(x * y for x, y in zip(a, b))


def normalize(text):
    out = []
    space = True
    for ch in text.lower():
        if ch.isalnum():
            out.append(ch)
            space = False
        elif not space:
            out.append(" ")
            space = True
    return "".join(out).strip()


def token_similarity(query, text):
    if query == text:
        return 1.0
    common = 0
    for left, right in zip(query, text):
        if left != right:
            break
        common += 1
    shorter = min(len(query), len(text))
    if shorter >= 5 and common >= 5:
        return common / max(len(query), len(text)) * 0.72
    return 0.0


def lexical_score(query, text):
    normalized_query = normalize(query)
    tokens = [
        token
        for token in normalized_query.split()
        if len(token) > 2
    ]
    if not tokens:
        return 0.0
    words = [word for word in normalize(text).split() if word]
    matched = sum(max((token_similarity(token, word) for word in words), default=0.0) for token in tokens)
    coverage = matched / len(tokens)
    phrase = 1.0 if normalized_query and normalized_query in normalize(text) else 0.0
    return min(1.0, coverage * 0.95 + phrase * 0.05)


def rank_fusion_metrics(facts, corpus, query_vectors, mode):
    r1 = r5 = r10 = 0
    reciprocal = 0.0
    count = 0
    for fact_id, fact, queries in facts:
        for query in queries:
            count += 1
            q = query_vectors[query]
            semantic = sorted(
                ((cosine(q, vector), fact[0]) for fact, vector in zip(facts, corpus)),
                key=lambda item: (-item[0], item[1]),
            )
            lexical = sorted(
                (
                    (lexical_score(query, candidate_fact), candidate[0])
                    for candidate, candidate_fact, _ in facts
                ),
                key=lambda item: (-item[0], item[1]),
            )
            semantic_rank = {fid: i for i, (_, fid) in enumerate(semantic)}
            lexical_rank = {fid: i for i, (_, fid) in enumerate(lexical)}
            fused = []
            for _, fid in semantic:
                sr = semantic_rank[fid] + 1
                lr = lexical_rank.get(fid)
                if mode == "rrf":
                    score = 1.0 / (60 + sr) + (1.0 / (60 + lr) if lr is not None else 0.0)
                elif mode == "weighted_rrf":
                    score = 2.0 / (60 + sr) + (1.0 / (60 + lr) if lr is not None else 0.0)
                elif mode == "lexical_boost":
                    score = 1.0 / (60 + sr) + 0.5 * (1.0 / (20 + lr) if lr is not None else 0.0)
                else:
                    score = -semantic[sr - 1][0]
                fused.append((score, fid))
            fused.sort(key=lambda item: (-item[0], item[1]))
            position = next((i for i, (_, fid) in enumerate(fused) if fid == fact_id), None)
            if position is not None:
                if position < 1:
                    r1 += 1
                if position < 5:
                    r5 += 1
                if position < 10:
                    r10 += 1
                reciprocal += 1 / (position + 1)
    return {
        "r@1": r1 / count,
        "r@5": r5 / count,
        "r@10": r10 / count,
        "mrr": reciprocal / count,
    }


def evaluate(model, facts, query_prefix="", passage_prefix=""):
    corpus = model.encode(
        [passage_prefix + fact for _, fact, _ in facts],
        normalize_embeddings=True,
        show_progress_bar=False,
    )
    all_queries = [query for _, _, queries in facts for query in queries]
    query_vectors = model.encode(
        [query_prefix + query for query in all_queries],
        normalize_embeddings=True,
        batch_size=32,
        show_progress_bar=False,
    )
    query_by_text = dict(zip(all_queries, query_vectors))
    r1 = r5 = r10 = 0
    reciprocal = 0.0
    count = 0
    misses = []
    for fact_id, _, queries in facts:
        for query in queries:
            count += 1
            q = query_by_text[query]
            ranked = sorted(
                ((cosine(q, vector), fact[0]) for fact, vector in zip(facts, corpus)),
                key=lambda item: (-item[0], item[1]),
            )
            position = next((i for i, (_, fid) in enumerate(ranked) if fid == fact_id), None)
            if position is not None:
                if position < 1:
                    r1 += 1
                if position < 5:
                    r5 += 1
                if position < 10:
                    r10 += 1
                reciprocal += 1 / (position + 1)
            else:
                misses.append((fact_id, query))
    return {
        "r@1": r1 / count,
        "r@5": r5 / count,
        "r@10": r10 / count,
        "mrr": reciprocal / count,
        "queries": count,
        "misses": misses,
        "fusion": {
            mode: rank_fusion_metrics(facts, corpus, query_by_text, mode)
            for mode in ("rrf", "weighted_rrf", "lexical_boost")
        },
        "score_grid": score_grid_metrics(facts, corpus, query_by_text),
    }


def score_grid_metrics(facts, corpus, query_vectors):
    results = []
    for alpha in (0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0):
        r1 = r5 = r10 = 0
        reciprocal = 0.0
        count = 0
        for fact_id, fact, queries in facts:
            for query in queries:
                count += 1
                q = query_vectors[query]
                ranked = sorted(
                    (
                        (
                            lexical_score(query, candidate_fact)
                            + alpha * cosine(q, vector),
                            candidate,
                        )
                        for (candidate, candidate_fact, _), vector in zip(facts, corpus)
                    ),
                    key=lambda item: (-item[0], item[1]),
                )
                position = next(
                    (i for i, (_, fid) in enumerate(ranked) if fid == fact_id),
                    None,
                )
                if position is not None:
                    if position < 1:
                        r1 += 1
                    if position < 5:
                        r5 += 1
                    if position < 10:
                        r10 += 1
                    reciprocal += 1 / (position + 1)
        results.append(
            {
                "alpha": alpha,
                "r@1": r1 / count,
                "r@5": r5 / count,
                "r@10": r10 / count,
                "mrr": reciprocal / count,
            }
        )
    return results


def main():
    models = sys.argv[1:] or [
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2",
        "intfloat/multilingual-e5-small",
        "sentence-transformers/paraphrase-multilingual-mpnet-base-v2",
    ]
    facts = load_facts()
    prefixes = {
        "intfloat/multilingual-e5-small": ("query: ", "passage: "),
        "intfloat/multilingual-e5-base": ("query: ", "passage: "),
        "intfloat/multilingual-e5-large": ("query: ", "passage: "),
    }
    results = []
    for model_id in models:
        print(f"loading {model_id}", flush=True)
        model = SentenceTransformer(model_id)
        query_prefix, passage_prefix = prefixes.get(model_id, ("", ""))
        metrics = evaluate(model, facts, query_prefix, passage_prefix)
        metrics["model"] = model_id
        dimension_fn = getattr(model, "get_embedding_dimension", None)
        if dimension_fn is None:
            dimension_fn = model.get_sentence_embedding_dimension
        metrics["dimension"] = dimension_fn()
        results.append(metrics)
        print(
            f"{model_id}: dim={metrics['dimension']} "
            f"R@1={metrics['r@1']:.3f} R@5={metrics['r@5']:.3f} "
            f"R@10={metrics['r@10']:.3f} MRR={metrics['mrr']:.3f}",
            flush=True,
        )
        for mode, fusion_metrics in metrics["fusion"].items():
            print(
                f"  fusion={mode} R@1={fusion_metrics['r@1']:.3f} "
                f"R@5={fusion_metrics['r@5']:.3f} R@10={fusion_metrics['r@10']:.3f} "
                f"MRR={fusion_metrics['mrr']:.3f}",
                flush=True,
            )
        for row in metrics["score_grid"]:
            print(
                f"  score_grid alpha={row['alpha']:.2f} R@1={row['r@1']:.3f} "
                f"R@5={row['r@5']:.3f} R@10={row['r@10']:.3f} MRR={row['mrr']:.3f}",
                flush=True,
            )
    out = ROOT / "docs" / "measurements" / "long_horizon_model_comparison.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
