# Templater

This is how I've automated my project setup.

Whenever I start a new project (as in: anything that has/needs a `flake.nix` at its folder top level), I use this as a base.

## Usage

```shell
nix run github:haglobah/templater -Lv -- --to <new_project_dir_name> <flag1> <flag2> <flag3>
# So, concretely:
nix run github:haglobah/templater -Lv -- --to my_webapp devshell cljs haskell
cd my_webapp
direnv allow
```

A word of caution: **This relentlessly overwrites anything that's in the target folder.**
Make sure you don't execute it on anything that's not version controlled already.
