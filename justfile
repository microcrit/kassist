compile *args:
    cargo run -p ezcompile -- {{args}}

compile-dev *args:
    cargo run -p ezcompile -- --dev {{args}}

flatpak-install *args:
    cargo run -p ezcompile -- flatpak install {{args}}

install *args:
    cargo run -p ezcompile -- install {{args}}

format:
    ruff check --fix --unsafe-fixes .
    ruff format .