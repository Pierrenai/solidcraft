# SolidCraft — CAO mécanique en Rust

Application Windows native, en français, pour concevoir des pièces destinées à l’impression 3D. L’interface, la géométrie, le rendu 3D et l’export sont écrits en Rust. Aucune connexion réseau n’est nécessaire à l’exécution.

## Télécharger

Télécharger la version Windows x86-64 depuis [les releases GitHub](https://github.com/Pierrenai/solidcraft/releases/latest).
Privilégier l’archive ZIP avec exemples et notices, puis lancer SolidCraft.exe après extraction.

## Licence

Copyright (C) 2026 Pierre (Pierrenai) et les contributeurs de SolidCraft.
SolidCraft est distribué sous **GNU GPL version 3 uniquement (GPL-3.0-only)** ; voir [LICENSE](LICENSE).
Vous pouvez utiliser, étudier, modifier et redistribuer le programme selon cette licence, sans garantie.
Les dépendances conservent leurs licences ; leurs textes sont fournis dans l’archive Windows.
Les instructions de compilation sont dans [BUILDING.md](BUILDING.md).

## Démarrer

Double-cliquer sur `SolidCraft.exe` (version 0.3) ou `Lancer SolidCraft.cmd`.

```sh
cargo run --release
```

## Concevoir une pièce (démarche SolidWorks / Creo)

1. **Fichier › Nouvelle pièce.** Les trois plans (Face, Dessus, Droite) s'affichent.
2. **Esquisse** : sélectionnez un plan ou une face plane (dans la vue ou l'arbre), puis Esquisse. La vue se met normale au plan.
3. Dessinez : **Ligne (L)**, **Rectangle (R)**, Rectangle centré, **Cercle (C)**, **Arc (A)**, Rainure, Ligne de construction. Les relations horizontale/verticale sont déduites (badges H / V), les points s'accrochent aux extrémités, milieux et entités.
4. **Cotation intelligente (D)** : cliquez une ligne, un cercle ou deux éléments, placez la cote, tapez la valeur (calculs acceptés : `40/2+3`). Bleu = libre, noir = totalement contraint. Double-clic sur une cote pour la modifier. En mode Sélection, glissez les points libres.
5. **Fonctions** : Bossage / Enlèvement extrudé (borgne, à travers tout, plan milieu), Révolution autour d'une ligne de construction, Congé et Chanfrein sur les arêtes cliquées, Assistance pour le perçage (M2 à M10 : passage, lamé, fraisé, taraudé), Répétitions linéaire et circulaire, Symétrie, Plan décalé. Prévisualisation jaune, flèche de profondeur à glisser, validation ✔ (Entrée) ou ✖ (Échap).
6. Lancer une fonction depuis une esquisse ferme l'esquisse et l'utilise. Lancer Bossage sans esquisse demande d'abord un plan.
7. **Arbre de construction** : double-clic = éditer ; clic droit = éditer l'esquisse, renommer, désactiver, supprimer, reprendre avant cette fonction (barre de reprise). Double-clic sur une face = éditer la fonction qui l'a créée. Une esquisse posée sur une face suit cette face si les cotes changent.
8. **Évaluer** : Mesurer (aire, longueur, distance, angle) et Propriétés de masse selon le matériau (clic droit sur Matériau : PLA, PETG, ABS…).
9. **Enregistrer** produit un projet `.solidcraft` ; **Exporter STL** pour le trancheur (unités : mm).

## Souris et clavier

- Survol : présélection orange de la face, de l'arête ou du plan ; clic : sélection bleue et barre contextuelle (Esquisse, Normal à, Éditer, Perçage, Congé…). Ctrl+clic : sélection multiple.
- Bouton du milieu : rotation · Ctrl+milieu ou clic droit glissé : déplacement · Maj+milieu : zoom · Molette : zoom vers le curseur · Glisser à gauche dans le vide : rotation · Flèches : rotation de 15°.
- F : zoom au cadre · Ctrl+1…7 : vues standard · Ctrl+8 : normal à · Espace : orientation.
- Entrée : valider · Échap : terminer l'outil / annuler · Suppr : supprimer · Ctrl+Z / Ctrl+Y · Ctrl+S · Ctrl+B : reconstruire.

## Limites de la version 0.3

- Noyau maillé (CSG BSP en double précision), pas un noyau B-Rep : cercles en 96 segments.
- Congés et chanfreins sur arêtes rectilignes entre faces planes ; les coins où trois congés se rejoignent ne sont pas arrondis en sphère.
- Pas d'outil Ajuster dans l'esquisse : les contours doivent être fermés sans se croiser (un contour intérieur fait un trou).
- Les fonctions de la version 0.2 (primitives, balayage, lissage, coque, assemblages, mise en plan) restent dans les onglets Primitives, Assemblage et Mise en plan.

## Développement

```sh
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --release
```

- `src/geometry.rs` : géométrie, triangulation, opérations booléennes.
- `src/model.rs` : objets paramétriques, validation des projets.
- `src/stl.rs` : export STL binaire.
- `src/part.rs` : arbre de fonctions paramétrique et reconstruction.
- `src/psketch.rs` : esquisses (entités, relations, cotes pilotantes, degrés de liberté).
- `src/topology.rs` : faces et arêtes retrouvées sur le maillage.
- `src/viewport.rs` : caméra, rendu logiciel, identifiants de face par pixel.
- `src/cad.rs`, `src/cad/panels.rs` : sélection, commandes, arbre et PropertyManager.
- `src/sketcher.rs` : outils d'esquisse interactifs.
- `src/main.rs` : fenêtre, ruban, raccourcis et fichiers.
- `tests/geometry.rs` : vérification des volumes, des projets et des STL.
- `prototype-web/` : première ébauche web conservée pour référence ; elle n’est pas utilisée par l’application Rust.

Interface construite avec [eframe / egui](https://docs.rs/eframe/0.31.1/eframe/).
