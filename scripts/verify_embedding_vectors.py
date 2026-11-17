#!/usr/bin/env python3
"""Verify a stored embedding index against an independent reference encoder.

Why this exists
---------------
The index header carries a provider/model fingerprint, but the fingerprint is
stamped from the *configuration*, not from the provider that actually ran.  A
build that fell back to the mock provider therefore produces an index that
looks healthy -- right width, matching fingerprint -- and contains hash
garbage.  Nothing in the header can tell the two apart; only a numeric
comparison against an independent encoder can.

What it checks
--------------
  * the index header (count, dim, fingerprint, unit norms)
  * that the stored vector of a sampled block reproduces a fresh encoding of
    the very same text, under both pooling conventions:
      - ``native``:  mean over content tokens, [CLS]/[SEP] excluded.  This is
        what the engine's native BERT provider does.
      - ``st_like``: mean over every real token, [CLS]/[SEP] included.  This is
        what ``SentenceTransformer.encode`` does, so the two agree at ~0.99
        instead of 1.0.

Usage
-----
    python scripts/verify_embedding_vectors.py --config <config.toml> [--sample 150]

Requires the reference model to be available locally
(``sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2``).
"""
from __future__ import annotations

import argparse
import re
import struct
import sys
from pathlib import Path

import numpy as np

EMB2_MAGIC = b"EMB2"
EMB_HEADER_V2 = 24

# BlockHeader v4 (MSC4): 32-byte spatial header + project_id(16) +
# importance + flags.  The config's `index.header_size` is not the on-disk
# size -- using the wrong one silently shifts data_offset/data_len.
BLOCK_HEADER = 50


def read_output_dir(config: Path) -> Path:
    text = config.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'^\s*output_dir\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        raise SystemExit(f"{config}: no output_dir")
    out = Path(m.group(1))
    return out if out.is_absolute() else config.parent / out


def read_index(path: Path):
    data = path.read_bytes()
    if data.startswith(EMB2_MAGIC):
        count, dim, max_depth = struct.unpack_from("<III", data, 4)
        fingerprint = struct.unpack_from("<Q", data, 16)[0]
        ids = np.frombuffer(data, dtype="<u4", count=count, offset=EMB_HEADER_V2)
        vecs = np.frombuffer(data, dtype="<f4", count=count * dim,
                             offset=EMB_HEADER_V2 + count * 4).reshape(count, dim)
        return ids, vecs, dim, max_depth, fingerprint
    count, dim, max_depth = struct.unpack_from("<III", data, 0)
    ids = np.frombuffer(data, dtype="<u4", count=count, offset=12)
    vecs = np.frombuffer(data, dtype="<f4", count=count * dim,
                         offset=12 + count * 4).reshape(count, dim)
    return ids, vecs, dim, max_depth, None


def block_texts(out_dir: Path, ids: np.ndarray):
    """Text of every indexed block, in index order.

    The vector at position ``i`` belongs to ``block_ids[i]`` -- not to the
    i-th text of some other ordering.  Pairing a scattered text with
    ``vecs[0..N]`` compares unrelated vectors and yields a meaningless
    cosine.
    """
    headers = (out_dir / "microscope.bin").read_bytes()
    data = (out_dir / "data.bin").read_bytes()
    n = len(headers) // BLOCK_HEADER
    out = []
    for b in ids.tolist():
        if b >= n:
            continue
        off = b * BLOCK_HEADER
        data_offset = struct.unpack_from("<I", headers, off + 18)[0]
        data_len = struct.unpack_from("<H", headers, off + 22)[0]
        end = min(data_offset + data_len, len(data))
        if data_offset >= end:
            continue
        try:
            out.append((b, data[data_offset:end].decode("utf-8")))
        except UnicodeDecodeError:
            continue
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--sample", type=int, default=150)
    ap.add_argument("--model",
                    default="sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2")
    a = ap.parse_args()

    config = Path(a.config).resolve()
    out_dir = read_output_dir(config)
    print(f"config : {config}")
    print(f"output : {out_dir}")

    ids, vecs, dim, max_depth, fingerprint = read_index(out_dir / "embeddings.bin")
    print(f"index  : count={len(ids)} dim={dim} max_depth={max_depth} "
          f"fingerprint={fingerprint}")
    norms = np.linalg.norm(vecs, axis=1)
    print(f"vectors: mean_norm={norms.mean():.4f} min={norms.min():.4f} "
          f"max={norms.max():.4f} (1.0 = normalised)")

    items = block_texts(out_dir, ids)
    if not items:
        raise SystemExit("no embedded blocks with readable text")
    print(f"texts  : {len(items)} embedded blocks carry text")

    try:
        import torch
        from tokenizers import Tokenizer
        from transformers import AutoModel
    except ImportError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    tok = Tokenizer.from_pretrained(a.model)
    model = AutoModel.from_pretrained(a.model)
    model.eval()

    rng = np.random.default_rng(0)
    take = min(a.sample, len(items))
    picks = sorted(rng.permutation(len(items))[:take].tolist())

    rows = []
    for pos in picks:
        block_id, text = items[pos]
        if not text.strip():
            continue
        enc = tok.encode(text)
        if not enc.ids:
            continue
        special = enc.special_tokens_mask
        real = [i for i, m in enumerate(enc.attention_mask) if m != 0]
        content = [i for i, s in enumerate(special) if s == 0]
        if not content:
            continue
        with torch.no_grad():
            hidden = model(input_ids=torch.tensor([enc.ids])).last_hidden_state[0].numpy()

        def unit(vector):
            norm = np.linalg.norm(vector)
            return vector / norm if norm else vector

        st_like = unit(hidden[real].mean(axis=0))
        native = unit(hidden[content].mean(axis=0))
        rows.append((
            len(text.encode("utf-8")),
            float(np.dot(vecs[pos], st_like)),
            float(np.dot(vecs[pos], native)),
        ))

    if not rows:
        raise SystemExit("no comparable rows")

    arr = np.array(rows)
    print(f"\nsample : {len(rows)} blocks")
    print(f"{'bytes':>16} {'n':>5} {'cos(st_like)':>13} {'cos(native)':>13}")
    edges = [0, 200, 400, 800, 1600, 4096, 1 << 30]
    for lo, hi in zip(edges, edges[1:]):
        sel = arr[(arr[:, 0] >= lo) & (arr[:, 0] < hi)]
        if len(sel) == 0:
            continue
        label = f"{lo}-{hi}" if hi < (1 << 30) else f"{lo}+"
        print(f"{label:>16} {len(sel):>5} {sel[:, 1].mean():>13.4f} {sel[:, 2].mean():>13.4f}")

    print(f"\nOVERALL mean cos  st_like={arr[:, 1].mean():.4f}  native={arr[:, 2].mean():.4f}")
    print(f"OVERALL min  cos  st_like={arr[:, 1].min():.4f}  native={arr[:, 2].min():.4f}")
    verdict = "REAL" if arr[:, 2].mean() > 0.9 else "MOCK/BROKEN"
    print(f"\nVERDICT: {verdict} (native-pool mean cosine {arr[:, 2].mean():.4f})")
    return 0 if verdict == "REAL" else 2


if __name__ == "__main__":
    raise SystemExit(main())
