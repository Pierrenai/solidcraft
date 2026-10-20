# Construire SolidCraft 0.3.0

Le code de cette version est disponible sur https://github.com/Pierrenai/solidcraft au tag v0.3.0.
Les sources correspondantes sont téléchargeables dans la même release (Source code).
Les versions des dépendances et leurs sources crates.io sont fixées par Cargo.lock.

## Windows x86-64

Installer Rust avec la cible MSVC et les outils de compilation C++ Visual Studio (Windows SDK inclus).
Depuis la racine du dépôt :

```powershell
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked
powershell -NoProfile -File scripts/package.ps1 -SkipBuild
```

L'exécutable est produit dans target/release/solidcraft.exe.
Le script de distribution crée release/SolidCraft-0.3.0-windows-x86_64.zip,
un EXE séparé et SHA256SUMS.txt. L'archive contient les exemples et les notices
des dépendances dans LICENSES/ et THIRD_PARTY.md.
Pour une copie de travail hors connexion, utiliser cargo vendor avec sa configuration indiquée
avant de déconnecter la machine.

Le modèle scripts/windows-workflow.yml.example configure les contrôles et la compilation Windows dans GitHub Actions.
Pour l’activer, copier ce fichier dans .github/workflows/windows.yml et pousser avec une autorisation GitHub incluant workflow.
Une fois activé, un tag v* publie également les fichiers dans une release GitHub.
La compilation locale publiée utilise la chaîne Rust installée ; la CI utilise Rust stable.
Les exécutables Windows ne sont pas signés numériquement.
