# Outils du projet Domino

## gen_avatars.py — génération des 20 avatars (Gemini)

Génère `web/avatars/avatar0.png` … `avatar19.png` : visages cartoon flat-design,
style cohérent avec le jeu (fond circulaire pastel, traits épais). Le script est
**idempotent** : les fichiers déjà présents sont sautés.

### Prérequis

- Clé API Google AI Studio (modèle `gemini-2.5-flash-image`).
- SDK Python : un venv avec `google-genai` existe dans `/tmp/opencode/gemini-env`
  (à recréer si /tmp a été vidé) :
  ```bash
  python3 -m venv /tmp/opencode/gemini-env
  /tmp/opencode/gemini-env/bin/pip install google-genai
  ```

### Lancer la génération

```bash
GEMINI_API_KEY="..." /tmp/opencode/gemini-env/bin/python tools/gen_avatars.py
```

> ⚠️ Le quota gratuit Gemini pour les images se renouvelle **toutes les 24 h**
> (reset quotidien ~9h). En cas d'erreur `RESOURCE_EXHAUSTED`, réessayer le lendemain.

### Après génération — étape OBLIGATOIRE

Les avatars sont embarqués dans le binaire **au compile** (`build.rs` lit
`web/avatars/*.png`). Il faut donc :

```bash
cargo build --release -p domino-server   # ou simplement ./serveur.sh restart (recompile si modifié)
```

Vérification : `curl -sI http://localhost:8080/avatars/avatar0.png` → 200.

Côté client, chaque joueur est affecté à un avatar par hash de son nom
(`avatarFor()` dans web/index.html). Si le PNG n'existe pas encore, l'initiale
colorée s'affiche (repli automatique via `onerror`).
