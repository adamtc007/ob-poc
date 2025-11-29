#!/usr/bin/env python3
"""
Generate embeddings for CSG metadata using Google Gemini or OpenAI.

Prerequisites:
    pip install google-generativeai psycopg2-binary python-dotenv

Usage:
    export GOOGLE_API_KEY=...
    python scripts/generate_csg_embeddings.py

    # Or with OpenAI:
    export OPENAI_API_KEY=sk-...
    export EMBEDDING_PROVIDER=openai
    python scripts/generate_csg_embeddings.py
"""

import json
import os
import time
from typing import Optional

import psycopg2
from dotenv import load_dotenv
from psycopg2.extras import execute_values

load_dotenv()

# Configuration
DATABASE_URL = os.getenv("DATABASE_URL", "postgresql:///data_designer?user=adamtc007")
GOOGLE_API_KEY = os.getenv("GOOGLE_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
EMBEDDING_PROVIDER = os.getenv("EMBEDDING_PROVIDER", "google")  # "google" or "openai"

# Model configs
GOOGLE_EMBEDDING_MODEL = "models/text-embedding-004"
GOOGLE_EMBEDDING_DIMENSIONS = 768
OPENAI_EMBEDDING_MODEL = "text-embedding-ada-002"
OPENAI_EMBEDDING_DIMENSIONS = 1536

BATCH_SIZE = 20
RATE_LIMIT_DELAY = 0.5  # seconds between batches


def get_embedding_google(client, text: str) -> list[float]:
    """Get embedding vector using Google Gemini."""
    result = client.embed_content(
        model=GOOGLE_EMBEDDING_MODEL, content=text, task_type="retrieval_document"
    )
    return result["embedding"]


def get_embedding_openai(client, text: str) -> list[float]:
    """Get embedding vector using OpenAI."""
    response = client.embeddings.create(model=OPENAI_EMBEDDING_MODEL, input=text)
    return response.data[0].embedding


def build_document_type_text(row: dict) -> str:
    """Build text representation for document type embedding."""
    parts = [
        f"Document type: {row['type_code']}",
        f"Display name: {row.get('display_name') or row['type_code']}",
    ]

    if row.get("semantic_context"):
        ctx = (
            row["semantic_context"]
            if isinstance(row["semantic_context"], dict)
            else json.loads(row["semantic_context"])
        )
        if ctx.get("purpose"):
            parts.append(f"Purpose: {ctx['purpose']}")
        if ctx.get("synonyms"):
            parts.append(f"Also known as: {', '.join(ctx['synonyms'])}")
        if ctx.get("keywords"):
            parts.append(f"Keywords: {', '.join(ctx['keywords'])}")

    if row.get("applicability"):
        app = (
            row["applicability"]
            if isinstance(row["applicability"], dict)
            else json.loads(row["applicability"])
        )
        if app.get("entity_types"):
            parts.append(f"Applicable to: {', '.join(app['entity_types'])}")
        if app.get("category"):
            parts.append(f"Category: {app['category']}")

    return "\n".join(parts)


def build_entity_type_text(row: dict) -> str:
    """Build text representation for entity type embedding."""
    parts = [
        f"Entity type: {row['type_code']}",
        f"Name: {row.get('name') or row['type_code']}",
    ]

    if row.get("type_hierarchy_path"):
        path = row["type_hierarchy_path"]
        if isinstance(path, list):
            parts.append(f"Hierarchy: {' > '.join(path)}")

    if row.get("semantic_context"):
        ctx = (
            row["semantic_context"]
            if isinstance(row["semantic_context"], dict)
            else json.loads(row["semantic_context"])
        )
        if ctx.get("category"):
            parts.append(f"Category: {ctx['category']}")
        if ctx.get("typical_documents"):
            parts.append(f"Typical documents: {', '.join(ctx['typical_documents'])}")
        if ctx.get("typical_attributes"):
            parts.append(f"Typical attributes: {', '.join(ctx['typical_attributes'])}")
        if ctx.get("synonyms"):
            parts.append(f"Also known as: {', '.join(ctx['synonyms'])}")

    return "\n".join(parts)


def generate_document_type_embeddings(conn, get_embedding, model_name: str):
    """Generate embeddings for all document types."""
    print("\n=== Generating Document Type Embeddings ===")

    with conn.cursor() as cur:
        # Get document types needing embeddings
        cur.execute("""
            SELECT type_id, type_code, display_name, applicability, semantic_context
            FROM "ob-poc".document_types
            WHERE applicability != '{}'::jsonb
              AND (embedding IS NULL
                   OR embedding_updated_at < NOW() - INTERVAL '7 days')
        """)
        rows = cur.fetchall()
        columns = [desc[0] for desc in cur.description]

        print(f"Found {len(rows)} document types to process")

        for i, row in enumerate(rows):
            row_dict = dict(zip(columns, row))
            text = build_document_type_text(row_dict)

            try:
                embedding = get_embedding(text)

                cur.execute(
                    """
                    UPDATE "ob-poc".document_types
                    SET embedding = %s::vector,
                        embedding_model = %s,
                        embedding_updated_at = NOW()
                    WHERE type_id = %s
                """,
                    (embedding, model_name, row_dict["type_id"]),
                )

                print(f"  [{i + 1}/{len(rows)}] {row_dict['type_code']} OK")

            except Exception as e:
                print(f"  [{i + 1}/{len(rows)}] {row_dict['type_code']} ERROR: {e}")

            if (i + 1) % BATCH_SIZE == 0:
                conn.commit()
                time.sleep(RATE_LIMIT_DELAY)

        conn.commit()


def generate_entity_type_embeddings(conn, get_embedding, model_name: str):
    """Generate embeddings for all entity types."""
    print("\n=== Generating Entity Type Embeddings ===")

    with conn.cursor() as cur:
        # Get entity types needing embeddings
        cur.execute("""
            SELECT entity_type_id, type_code, name, type_hierarchy_path, semantic_context
            FROM "ob-poc".entity_types
            WHERE type_code IS NOT NULL
              AND type_hierarchy_path IS NOT NULL
              AND (embedding IS NULL
                   OR embedding_updated_at < NOW() - INTERVAL '7 days')
        """)
        rows = cur.fetchall()
        columns = [desc[0] for desc in cur.description]

        print(f"Found {len(rows)} entity types to process")

        for i, row in enumerate(rows):
            row_dict = dict(zip(columns, row))
            text = build_entity_type_text(row_dict)

            try:
                embedding = get_embedding(text)

                cur.execute(
                    """
                    UPDATE "ob-poc".entity_types
                    SET embedding = %s::vector,
                        embedding_model = %s,
                        embedding_updated_at = NOW()
                    WHERE entity_type_id = %s
                """,
                    (embedding, model_name, row_dict["entity_type_id"]),
                )

                print(f"  [{i + 1}/{len(rows)}] {row_dict['type_code']} OK")

            except Exception as e:
                print(f"  [{i + 1}/{len(rows)}] {row_dict['type_code']} ERROR: {e}")

            if (i + 1) % BATCH_SIZE == 0:
                conn.commit()
                time.sleep(RATE_LIMIT_DELAY)

        conn.commit()


def populate_similarity_cache(conn):
    """Populate the semantic similarity cache."""
    print("\n=== Populating Similarity Cache ===")

    with conn.cursor() as cur:
        # Check if vector extension is available
        cur.execute("""
            SELECT EXISTS (
                SELECT 1 FROM pg_extension WHERE extname = 'vector'
            )
        """)
        has_vector = cur.fetchone()[0]

        if not has_vector:
            print("pgvector extension not installed, skipping similarity cache")
            return

        # Document type similarities
        print("Computing document type similarities...")
        cur.execute("""
            INSERT INTO "ob-poc".csg_semantic_similarity_cache
                (source_type, source_code, target_type, target_code,
                 cosine_similarity, computed_at, expires_at)
            SELECT
                'document_type', d1.type_code,
                'document_type', d2.type_code,
                1 - (d1.embedding <=> d2.embedding) as similarity,
                NOW(), NOW() + INTERVAL '7 days'
            FROM "ob-poc".document_types d1
            CROSS JOIN "ob-poc".document_types d2
            WHERE d1.type_id < d2.type_id
              AND d1.embedding IS NOT NULL
              AND d2.embedding IS NOT NULL
              AND 1 - (d1.embedding <=> d2.embedding) > 0.5
            ON CONFLICT (source_type, source_code, target_type, target_code)
            DO UPDATE SET
                cosine_similarity = EXCLUDED.cosine_similarity,
                computed_at = NOW(),
                expires_at = NOW() + INTERVAL '7 days'
        """)
        print(f"  Inserted/updated {cur.rowcount} document similarity records")

        # Entity type similarities
        print("Computing entity type similarities...")
        cur.execute("""
            INSERT INTO "ob-poc".csg_semantic_similarity_cache
                (source_type, source_code, target_type, target_code,
                 cosine_similarity, computed_at, expires_at)
            SELECT
                'entity_type', e1.type_code,
                'entity_type', e2.type_code,
                1 - (e1.embedding <=> e2.embedding) as similarity,
                NOW(), NOW() + INTERVAL '7 days'
            FROM "ob-poc".entity_types e1
            CROSS JOIN "ob-poc".entity_types e2
            WHERE e1.entity_type_id < e2.entity_type_id
              AND e1.embedding IS NOT NULL
              AND e2.embedding IS NOT NULL
              AND e1.type_code IS NOT NULL
              AND e2.type_code IS NOT NULL
              AND 1 - (e1.embedding <=> e2.embedding) > 0.5
            ON CONFLICT (source_type, source_code, target_type, target_code)
            DO UPDATE SET
                cosine_similarity = EXCLUDED.cosine_similarity,
                computed_at = NOW(),
                expires_at = NOW() + INTERVAL '7 days'
        """)
        print(f"  Inserted/updated {cur.rowcount} entity similarity records")

        conn.commit()


def main():
    # Determine which provider to use
    use_google = GOOGLE_API_KEY and EMBEDDING_PROVIDER != "openai"
    use_openai = OPENAI_API_KEY and not use_google

    if not use_google and not use_openai:
        print("WARNING: No API key set")
        print("Set GOOGLE_API_KEY or OPENAI_API_KEY")
        print("\nContinuing with similarity cache population only...")

        conn = psycopg2.connect(DATABASE_URL)
        try:
            populate_similarity_cache(conn)
            print("\n=== Complete (without embeddings) ===")
        finally:
            conn.close()
        return

    print(f"Connecting to database: {DATABASE_URL}")
    conn = psycopg2.connect(DATABASE_URL)

    try:
        if use_google:
            import google.generativeai as genai

            genai.configure(api_key=GOOGLE_API_KEY)
            print(f"Using Google Gemini ({GOOGLE_EMBEDDING_MODEL})")

            get_embedding = lambda text: get_embedding_google(genai, text)
            model_name = GOOGLE_EMBEDDING_MODEL
        else:
            from openai import OpenAI

            client = OpenAI(api_key=OPENAI_API_KEY)
            print(f"Using OpenAI ({OPENAI_EMBEDDING_MODEL})")

            get_embedding = lambda text: get_embedding_openai(client, text)
            model_name = OPENAI_EMBEDDING_MODEL

        generate_document_type_embeddings(conn, get_embedding, model_name)
        generate_entity_type_embeddings(conn, get_embedding, model_name)
        populate_similarity_cache(conn)

        print("\n=== Complete ===")

    finally:
        conn.close()


if __name__ == "__main__":
    main()
