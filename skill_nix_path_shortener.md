# Skill: Nix Path Shortener

This skill filters the context window and system prompts to replace verbose, hashed Nix store paths with shortened, stable aliases.

## Logic: `shorten_nix_paths`
When the agent encounters an absolute path starting with `/nix/store/`, it should automatically translate it in its internal reasoning to the project-relative stable symlink.

### Mapping Rule
- `/nix/store/<hash>-pi-<version>/lib/node_modules/pi/` -> `~/.pi/pi-source`
- Any `/nix/store/<hash>-<name>-<version>/` -> `[NIX-STORE]/<name>`

### Purpose
To minimize context token waste and prevent the LLM from becoming distracted by random cryptographic hashes that carry no semantic value for application logic.
