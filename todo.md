# OpenSnake — TODO

> Liste des améliorations du moteur, classées par priorité.
> Légende : ✅ fait · ⬜ à faire · priorité **Critique** > **Haute** > **Moyenne**.

## Déjà en place

| Statut | Fonctionnalité | Détail | Priorité |
|:---:|---|---|---|
| ✅ | Recherche | Iterative deepening, alpha-beta, PVS, quiescence, LMR, futility, null move, aspiration, check extensions | Critique |
| ✅ | Ordonnancement | TT move, MVV-LVA + SEE, killer moves, historique | Haute |
| ✅ | Table de transposition | Hash Zobrist, bornes exact / lower / upper, remplacement et redimensionnement | Haute |
| ✅ | UCI | `uci`, `isready`, `ucinewgame`, `position`, `go`, `stop`, `quit`, MultiPV, Hash, AnalyseMode, Cute Chess compatible | Critique |
| ✅ | Gestion du temps | `wtime`, `btime`, `winc`, `binc`, `movetime` | Critique |
| ✅ | Option ELO | 1–9999 | Haute |
| ✅ | Évaluation | Matériel + tables case-pièce + structure/tactique, interpolation MG/EG | Haute |
| ✅ | FEN | Import / export | Haute |
| ✅ | Tests & bench | Tests unitaires + intégration, tactique, position, self-play, session Cute Chess, commande `bench` (perft) | Haute |

## 1. Recherche

| Statut | Fonctionnalité | Description | Priorité |
|:---:|---|---|---|
| ✅ | Iterative Deepening | Chercher progressivement à profondeur 1, 2, 3... jusqu'à la limite de temps. | Critique |
| ✅ | Quiescence Search | Continuer à analyser les captures et positions tactiques avant d'évaluer une position. | Critique |
| ✅ | Principal Variation Search (PVS) | Optimiser alpha-beta en analysant les meilleurs coups plus précisément. | Critique |
| ✅ | Null Move Pruning | Éliminer certaines branches en testant un coup fictif. | Haute |
| ✅ | Late Move Reduction (LMR) | Réduire la profondeur des coups moins prometteurs. | Haute |
| ✅ | Futility Pruning | Éviter certaines recherches qui ne peuvent probablement pas améliorer le résultat. | Haute |
| ✅ | Aspiration Windows | Réduire la fenêtre alpha-beta autour de la précédente évaluation. | Haute |
| ✅ | Check Extensions | Approfondir certaines positions où le roi est en échec. | Haute |

## 2. Ordonnancement des coups

L'ordre dans lequel le moteur teste les coups est extrêmement important.

| Statut | Technique | Description | Priorité |
|:---:|---|---|---|
| ✅ | MVV-LVA | Privilégier les captures intéressantes. | Haute |
| ✅ | Killer Moves | Mémoriser les coups qui ont provoqué des coupures alpha-beta. | Haute |
| ✅ | History Heuristic | Apprendre quels coups sont généralement intéressants. | Moyenne |
| ✅ | SEE (Static Exchange Evaluation) | Estimer si une séquence de captures est avantageuse (ordonnancement + quiescence). | Haute |
| ✅ | TT Move Ordering | Essayer en premier le meilleur coup trouvé dans la table de transposition. | Haute |

## 3. Évaluation

L'évaluation (`src/engine.rs`) combine matériel, tables case-pièce et termes positionnels, interpolés entre milieu de jeu et finale. Reste à calibrer les poids avec des parties de test.

| Statut | Évaluation | Ce qu'elle analyse | Priorité |
|:---:|---|---|---|
| ✅ | Material Evaluation | Valeur des pièces. | Critique |
| ✅ | Piece-Square Tables | Position de chaque pièce (tables générées, miroir pour les Noirs). | Haute |
| ✅ | Pawn Structure | Pions isolés, doublés, passés. | Haute |
| ✅ | King Safety | Bouclier de pions et proximité de la dame adverse. | Haute |
| ✅ | Mobility | Nombre de cases accessibles. | Moyenne |
| ✅ | Bishop Pair | Bonus pour les deux fous. | Moyenne |
| ✅ | Passed Pawns | Bonus pour les pions passés. | Moyenne |
| ✅ | Rook Files | Tours sur colonnes ouvertes et semi-ouvertes. | Moyenne |
| ✅ | Threat Evaluation | Menaces contre les pièces adverses. | Haute |
| ✅ | Tapered Eval (MG/EG) | Deux scores (milieu / finale) interpolés selon la phase. | Haute |

Évaluation par phase de jeu :

- **Opening** : développement et contrôle du centre.
- **Middlegame** : sécurité du roi, activité des pièces.
- **Endgame** : activité du roi, promotion des pions.

## 4. NNUE

⬜ **NNUE (Efficiently Updatable Neural Network)** — *très important*

Technologie utilisée par Stockfish pour évaluer les positions avec un réseau de neurones spécialisé.

Au lieu de simplement utiliser des règles comme :

```rust
score += material_value;
score += mobility_bonus;
score += king_safety;
```

…on utilise un réseau neuronal entraîné sur des positions d'échecs.

**Avantages :**

- Évaluations beaucoup plus sophistiquées.
- Meilleure compréhension des positions.
- Meilleures décisions positionnelles.
- Possibilité d'améliorer le moteur avec des données d'entraînement.

**Approche conseillée :** commencer par une évaluation classique, puis implémenter NNUE plus tard.

## 5. Performance / architecture

Rust est rapide, mais l'architecture peut encore être optimisée.

| Statut | Optimisation | Détail | Priorité |
|:---:|---|---|---|
| ⬜ | Bitboards | Représenter l'échiquier avec des entiers de 64 bits. | Critique |
| ⬜ | Magic Bitboards | Accélérer la génération des coups des tours et des fous. | Critique |
| ⬜ | Incremental Evaluation | Mettre à jour le score sans tout recalculer. | Haute |
| ✅ | Zobrist Hashing | Identifier rapidement les positions. | Critique |
| ✅ | Transposition Table | Cache des positions (profondeur, score, borne, meilleur coup). | Critique |
| ✅ | Optimized TT | Remplacement par profondeur, éviction des entrées peu profondes, taille configurable. | Haute |
| ⬜ | Multithreading | Utiliser plusieurs cœurs CPU (principal + split-search). | Haute |
| ⬜ | SIMD | Accélérer certains calculs numériques (AVX2 / AVX-512). | Moyenne |

## 6. UCI & fonctionnalités

| Statut | Fonctionnalité | Utilité | Priorité |
|:---:|---|---|---|
| ✅ | MultiPV | Afficher les N meilleures variantes (`info ... multipv`). | Haute |
| ✅ | MoveDelay / timing ELO | Délai de réflexion dérivé de l'ELO (bas ELO plus lent), configurable en ms + plage aléatoire, plafonné pour éviter les pertes au temps. | Haute |
| ✅ | UCI_AnalyseMode | Mode analyse sans contrainte de temps ni erreur ELO. | Moyenne |
| ✅ | UCI_Elo | Limiter la force du moteur (1–9999). | Haute |
| ✅ | Hash option | Configurer la mémoire utilisée (1–1024 Mo). | Moyenne |
| ⬜ | Threads option | Option acceptée mais recherche encore mono-thread. | Moyenne |
| ⬜ | Syzygy Tablebases | Jouer parfaitement certaines fins de partie. | Moyenne |
| ⬜ | Opening Book | Connaître des ouvertures pré-calculées (PolyGlot). | Moyenne |
| ⬜ | Ponder | Réfléchir pendant le tour adverse. | Moyenne |
| ⬜ | WDL Output | Afficher les probabilités Win / Draw / Loss. | Moyenne |

## 7. Tests & validation

Un moteur qui compile et trouve un coup légal n'est pas forcément un bon moteur.

| Statut | Test | Objectif | Priorité |
|:---:|---|---|---|
| ✅ | Tests unitaires / intégration | UCI, moteur, table de transposition. | Critique |
| ✅ | Perft (`bench`) | Vérifier la génération des coups légaux et mesurer les nodes/sec. | Critique |
| ✅ | Tactical Tests | SEE gagnant/perdant et mat en un trouvé par la recherche. | Haute |
| ✅ | Position Tests | Cavalier central, pion passé, couple de fous. | Haute |
| ✅ | Self-play | Séquence de coups légaux contre lui-même (ELO bas). | Moyenne |
| ⬜ | SPRT | Mesurer statistiquement si une version est plus forte. | Moyenne |
| ⬜ | ELO Testing | Mesurer sa force contre d'autres moteurs. | Moyenne |

✅ **Compatibilité Cute Chess validée** : handshake asynchrone (`uci` → `uciok`, `isready` → `readyok` même pendant la recherche), `ucinewgame`, `stop` pendant `go infinite`, gestion de `wtime`/`btime`/`winc`/`binc`/`movestogo`/`movetime`, et tests d'intégration `tests/cutechess.rs` qui pilent le vrai `.exe`. Self-play réel avec `baron30.bin` + `-repeat` (couleurs inversées) fonctionne. Correction au passage : les droits de roque n'étaient pas conservés après un coup normal, ce qui désynchronisait le moteur et produisait des coups illégaux (`tests/castling.rs`). Outil utilisé pour organiser des tournois entre moteurs.

> Comparer chaque nouvelle version à la précédente. Une modification qui semble intelligente peut en réalité faire perdre des Elo.

### Calibration de l'option ELO

`setoption name Elo value 1500` est une bonne idée, mais un moteur ne devient pas automatiquement un moteur à 1500 Elo en changeant simplement une valeur. Il faut contrôler sa force via :

- la réduction de la profondeur de recherche ;
- la probabilité de sélectionner un coup moins bon ;
- la limitation du temps de réflexion ;
- la réduction de la précision de recherche.

Et idéalement, calibrer ces paramètres avec des milliers de parties.

✅ **Plage ELO 1–9999 validée** : tests qui balayent **chaque valeur** de 1 à 9999 et vérifient que profondeur, probabilité d'erreur, `top_n` et délai restent dans les bornes et évoluent toujours dans le bon sens. Le parser UCI accepte aussi toutes les valeurs 1–9999 (hors bornes ramenés à 1 ou 9999).

## 8. Roadmap

| Horizon | Objectifs | Impact | Statut |
|:---|---|---|---|
| **Court terme** | Quiescence Search ✅ + Null Move Pruning ✅ + Aspiration ✅ | Gains de force les plus immédiats | Terminé |
| **Moyen terme** | PVS ✅, LMR ✅, meilleure table de transposition ✅, ordonnancement des coups ✅, évaluation classique ✅ | L'essentiel de l'Elo | En cours |
| **Long terme** | NNUE ⬜ | Le saut technologique qui sépare les moteurs « classiques » des moteurs « modernes » | À venir |
