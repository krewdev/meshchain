# MeshChain on X

| File | Purpose |
|------|---------|
| [PROFILE.md](./PROFILE.md) | Create account + API keys |
| [queue.json](./queue.json) | Ready posts (Day 1–7) |
| [../../scripts/x_post.py](../../scripts/x_post.py) | Poster CLI |
| [../../.github/workflows/x-post.yml](../../.github/workflows/x-post.yml) | Optional daily automation |

## Quick start

```bash
# 1) Create @MeshChain — see PROFILE.md
# 2) Put API keys in .env.x
source .env.x

python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1 --dry-run
python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1
```

State of what’s already posted: `data/x_post_state.json` (gitignored under `/data`).
