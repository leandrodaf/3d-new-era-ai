"""Example plugin: a table of room areas.

Reads the home from GET /api/home and inserts a text to the right of the plan
with one line per room and the total, as a single undoable edit through
POST /api/commands. Uses only the Python standard library.

Arguments (JSON on standard input, all optional):
  {"size": 20}   text height in cm
"""

import json
import os
import sys
import urllib.error
import urllib.request

URL = os.environ["NEWERA_URL"]
TOKEN = os.environ.get("NEWERA_TOKEN")
SESSION = os.environ.get("NEWERA_SESSION")


def call(path, body=None):
    request = urllib.request.Request(URL + path, method="POST" if body is not None else "GET")
    request.add_header("content-type", "application/json")
    if TOKEN:
        request.add_header("authorization", "Bearer " + TOKEN)
    data = json.dumps(body).encode() if body is not None else None
    with urllib.request.urlopen(request, data) as response:
        return json.load(response)


def area(points):
    """Polygon area in cm² (shoelace formula)."""
    total = 0.0
    for (x1, y1), (x2, y2) in zip(points, points[1:] + points[:1]):
        total += x1 * y2 - x2 * y1
    return abs(total) / 2


def next_id(home):
    """Ids share one counter: the next one is the largest suffix + 1."""
    largest = 0
    for value in home.values():
        if isinstance(value, list):
            for element in value:
                if isinstance(element, dict) and isinstance(element.get("id"), str):
                    digits = "".join(c for c in element["id"] if c.isdigit())
                    largest = max(largest, int(digits or 0))
    return largest + 1


def main():
    raw = sys.stdin.read().strip()
    args = json.loads(raw) if raw and raw != "null" else {}
    size = float(args.get("size", 20))

    state = call("/api/home")
    home = state["home"]
    rooms = [r for r in home.get("rooms", []) if len(r.get("points", [])) >= 3]
    if not rooms:
        print("no rooms on the plan")
        return

    lines = [f"{r.get('name') or 'Room'}: {area(r['points']) / 10_000:.2f} m²" for r in rooms]
    total = sum(area(r["points"]) for r in rooms) / 10_000
    lines.append(f"Total: {total:.2f} m²")

    xs = [p[0] for r in rooms for p in r["points"]]
    ys = [p[1] for r in rooms for p in r["points"]]
    label = {
        "kind": "label",
        "id": f"t{next_id(home)}",
        "text": "Room areas\n" + "\n".join(lines),
        "position": [max(xs) + 150, (min(ys) + max(ys)) / 2],
        "size": size,
        "align": "left",
    }
    body = {
        "commands": [{"op": "insert", "element": label}],
        "base_revision": state["revision"],
        "session": SESSION,
    }
    try:
        result = call("/api/commands", body)
    except urllib.error.HTTPError as error:
        print(error.read().decode(), file=sys.stderr)
        sys.exit(1)
    print(f"table with {len(rooms)} rooms ({total:.2f} m²), revision {result['revision']}")


if __name__ == "__main__":
    main()
