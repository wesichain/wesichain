import time
from langchain_community.embeddings import OllamaEmbeddings


def main():
    docs = [f"document {i} about rust" for i in range(1000)]
    embedder = OllamaEmbeddings(model="nomic-embed-text")
    start = time.time()
    vectors = embedder.embed_documents(docs)
    print("embed_ms", int((time.time() - start) * 1000))
    print("vectors", len(vectors))


if __name__ == "__main__":
    main()
