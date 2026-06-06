from fastapi.testclient import TestClient


def test_health(client: TestClient) -> None:
    resp = client.get("/api/health")
    assert resp.status_code == 200
    assert resp.json() == {"status": "healthy"}


def test_list_seeded_documents(client: TestClient) -> None:
    resp = client.get("/api/documents")
    assert resp.status_code == 200
    body = resp.json()
    assert body["total"] == 2
    names = {d["name"] for d in body["documents"]}
    assert names == {"README.md", "notes.txt"}


def test_search_matches_body(client: TestClient) -> None:
    resp = client.get("/api/documents/search", params={"query": "rotate"})
    assert resp.status_code == 200
    body = resp.json()
    assert [d["name"] for d in body["documents"]] == ["notes.txt"]


def test_get_and_delete_round_trip(client: TestClient) -> None:
    listed = client.get("/api/documents").json()
    doc_id = listed["documents"][0]["id"]

    fetched = client.get(f"/api/documents/{doc_id}")
    assert fetched.status_code == 200

    removed = client.delete(f"/api/documents/{doc_id}")
    assert removed.status_code == 204

    missing = client.get(f"/api/documents/{doc_id}")
    assert missing.status_code == 404
