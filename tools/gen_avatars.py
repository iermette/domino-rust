#!/usr/bin/env python3
"""Génère 20 avatars de joueurs pour le jeu de domino (Gemini Nano Banana)."""
import os, sys, time

from google import genai
from google.genai import types

OUT = "/home/tof/Téléchargements/Domino rust/crates/domino-server/web/avatars"
os.makedirs(OUT, exist_ok=True)

BASE = (
    "Flat vector illustration in a cute casual board-game style. "
    "Portrait of {desc}, head and shoulders only, perfectly centered, facing the viewer, "
    "friendly warm smile, soft rounded shapes, clean minimal details, smooth shading, "
    "bright cheerful colors, plain solid cream background (#f5f1e8), no text, no watermark. "
    "Consistent cartoon style of a friendly family game app."
)

CHARACTERS = [
    "a young man with short brown hair and a blue t-shirt",
    "a young woman with long blonde hair and a red top",
    "a middle-aged man with a gray stubble beard and a green sweater",
    "a middle-aged woman with curly black hair and an orange blouse",
    "an elderly man with a white mustache, round glasses and a beige cardigan",
    "an elderly woman with a silver bob haircut and a purple scarf",
    "a boy with messy red hair and a yellow t-shirt",
    "a girl with two black pigtails and a pink dress",
    "a man with dark curly hair, a gold hoop earring and a teal shirt",
    "a woman with long wavy auburn hair and a denim jacket",
    "a bald man with a big smile and a red-and-white striped polo shirt",
    "a woman with a short dark bob haircut and a mustard cardigan",
    "a young man with black dreadlocks and a gray hoodie",
    "a woman with a silver-gray ponytail and a lavender top",
    "a man with a brown flat cap and a plaid flannel shirt",
    "a girl with a black beanie, freckles and a green jacket",
    "a man with long blond hair tied in a bun and a gray t-shirt",
    "a woman with short red curly hair, round glasses and a blue dress",
    "a boy with a big dark afro and an orange soccer jersey",
    "a man with a black mustache, side-parted hair and a brown vest",
]

def main():
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        print("ERREUR : GEMINI_API_KEY absente"); sys.exit(1)
    client = genai.Client(api_key=api_key)

    for i, desc in enumerate(CHARACTERS):
        out_path = os.path.join(OUT, f"avatar{i}.png")
        if os.path.exists(out_path) and os.path.getsize(out_path) > 5000:
            print(f"[{i+1}/20] existe déjà — saut", flush=True); continue
        prompt = BASE.format(desc=desc)
        for attempt in range(4):
            try:
                resp = client.models.generate_content(
                    model="gemini-2.5-flash-image",
                    contents=prompt,
                    config=types.GenerateContentConfig(
                        response_modalities=["IMAGE"],
                        image_config=types.ImageConfig(aspect_ratio="1:1"),
                    ),
                )
                data = None
                if resp.candidates and resp.candidates[0].content.parts:
                    for part in resp.candidates[0].content.parts:
                        if getattr(part, "inline_data", None) is not None:
                            data = part.inline_data.data; break
                if not data:
                    raise RuntimeError("aucune image dans la réponse")
                with open(out_path, "wb") as f:
                    f.write(data)
                print(f"[{i+1}/20] OK {len(data)//1024} Ko — {desc[:50]}", flush=True)
                break
            except Exception as e:
                wait = 15 * (attempt + 1)
                print(f"[{i+1}/20] essai {attempt+1} échoué ({e}) — retry dans {wait}s", flush=True)
                time.sleep(wait)
        else:
            print(f"[{i+1}/20] ÉCHEC DÉFINITIF : {desc}", flush=True)
        time.sleep(8)  # respect du rate limit

    ok = sum(1 for i in range(20) if os.path.exists(os.path.join(OUT, f"avatar{i}.png")))
    print(f"TERMINÉ : {ok}/20 avatars générés", flush=True)

if __name__ == "__main__":
    main()
