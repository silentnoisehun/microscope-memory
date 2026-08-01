"""Microscope Memory embedding server — called as subprocess for batch embedding."""
import json, struct, sys, os
from sentence_transformers import SentenceTransformer

def main():
    model_name = sys.argv[1] if len(sys.argv) > 1 else "all-MiniLM-L6-v2"
    model = SentenceTransformer(model_name)
    dim = model.get_sentence_embedding_dimension()
    
    # Write header: dimension as 4 bytes (u32)
    sys.stdout.buffer.write(struct.pack("<I", dim))
    sys.stdout.buffer.flush()
    
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        emb = model.encode(line)
        sys.stdout.buffer.write(struct.pack(f"<{dim}f", *emb.tolist()))
        sys.stdout.buffer.flush()

if __name__ == "__main__":
    main()
