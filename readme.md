# Xiangqi: A Modern Chinese Chess Engine in Rust

A powerful, high-performance Xiangqi AI built with modern chess engine techniques in Rust.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Keywords**: Chinese Chess, Xiangqi, Chess Engine, AI, Rust, Bitboard, NegaMax, Alpha-Beta Pruning, Zobrist Hashing, Transposition Table

---

### English

**Xiangqi** is a high-performance Chinese Chess (Xiangqi) engine built from the ground up in Rust. It began as a learning project and has evolved to incorporate a wide range of advanced techniques found in state-of-the-art chess engines, with a strong focus on performance and efficiency.

This project serves not only as a capable Xiangqi AI but also as an excellent educational resource, demonstrating the complete journey of building a chess AI from basic algorithms to sophisticated optimizations in a modern, compiled language.

### 简体中文

**Xiangqi (象棋)** 是一个从零开始、使用 Rust 语言逐步构建的现代化、高性能中国象棋引擎。项目最初旨在学习和实践棋类 AI 算法，随着研究的深入，我们不断融入多种现代象棋引擎的核心技术，并充分利用 Rust 的性能优势，使其具备了强大的对弈能力与精准的评估体系。

该项目不仅是一个功能强大的象棋 AI，也是一个绝佳的学习资源，清晰地展示了棋类 AI 从基础算法到高级优化的完整实现过程。

---

## Core Technologies & Features (核心技术与特性)

The engine implements a comprehensive suite of technologies that form the backbone of modern chess programs.

| Feature | Description (English) | 描述 (中文) |
| :--- | :--- | :--- |
| **Board Representation** | **128-bit Bitboard**: Utilizes Rust's native `u128` integer type to represent the 90-square Xiangqi board, enabling highly efficient and fast bitwise operations for move generation and board manipulation. | **128位位棋盘**: 利用 Rust 的原生 `u128` 整数类型来表示 90 格的象棋棋盘，为走法生成和棋盘操作提供了高效、快速的位运算能力。 |
| **Search Algorithm** | **Iterative Deepening NegaMax with Alpha-Beta Pruning**: A highly efficient, layered search algorithm that minimizes the number of nodes to be evaluated in the search tree and allows for effective time management. | **迭代深化 NegaMax 搜索与 Alpha-Beta 剪枝**: 高效的、逐层加深的搜索算法，通过剪枝极大减少需要评估的节点数量，并便于时间控制。 |
| **Search Extensions** | **Quiescence Search**: Extends the search for captures after reaching the nominal depth, mitigating the "horizon effect" and stabilizing evaluations. | **静态搜索**: 在达到预设深度后继续扩展吃子着法，直至局面稳定，有效缓解“地平线效应”。 |
| **Search Optimizations** | **Null Move Pruning**: A technique that prunes branches of the search tree by assuming the opponent makes a "null move," which can quickly identify positions that are much worse than expected. | **空着裁剪**: 一种通过假设对手进行“空着”（跳过回合）来修剪搜索树分支的技术，可以快速识别必败局面。 |
| **Search Optimizations** | **Late Move Reduction (LMR)**: Reduces the search depth for moves that are ordered later, assuming they are less likely to be good. The search is re-run at full depth if the move proves to be surprisingly strong. | **迟着削减 (LMR)**: 对排序靠后的着法降低搜索深度，如果发现该着法优于预期，则以完整深度重新搜索，显著提升搜索效率。 |
| **Transposition Table**| **Zobrist Hashing & Transposition Table**: Uses Zobrist keys to store previously evaluated positions in a large transposition table, avoiding redundant calculations and enabling faster search. | **Zobrist 哈希与置换表**: 使用 Zobrist 键将已评估过的局面存入置换表，避免重复计算，显著提升搜索效率。 |
| **Move Ordering** | **Advanced Move Ordering**: Prioritizes moves to improve pruning efficiency: 1. TT Best Move, 2. Captures (MVV-LVA), 3. Quiet moves with high scores from the **History Heuristic**. | **高效着法排序**: 优先考虑置换表中的历史最佳着法、吃子着法 (MVV-LVA) 以及历史启发分数高的静默着法，以实现更频繁、更深度的剪枝。 |
| **Repetition Detection**| **Repetition Prevention & Detection**: Utilizes a history of Zobrist hashes to detect repeated positions and enforce draw rules, preventing infinite loops. | **循环检测与防止**: 利用哈希历史判定重复局面，并赋予和棋结果，避免无限循环。 |
| **Opening Book** | **Binary Opening Book**: Utilizes a pre-computed `opening_book.bin` to play standard openings, ensuring a strong and fast start. | **二进制开局库**: 在开局阶段直接从 `opening_book.bin` 文件中检索预设着法，保证开局质量和速度。 |
| **Evaluation** | **Tapered Evaluation with PST**: Employs two sets of Piece-Square Tables (PST) for middlegame and endgame. The evaluation dynamically blends these tables based on the game phase for a more nuanced understanding of piece values. | **渐进式评估与棋子位置表 (PST)**: 采用中局与残局两套位置表，根据场上子力动态混合评估结果，实现更精确的“棋感”。 |
| **Evaluation Features**| **Mobility, Patterns & King Safety**: The evaluation function considers piece mobility, common tactical patterns (e.g., "Bottom Cannon"), and king safety (e.g., missing guards, attacks on the palace). | **机动性、棋形与将帅安全**: 评估函数综合考量棋子活跃度、常见战术棋形（如“底炮”）以及将帅安全性（如缺士、九宫受攻击）。 |
| **Performance** | **Incremental Updates**: The board state, including evaluation scores and Zobrist hash, is updated incrementally with each move, avoiding costly recalculations from scratch. | **增量更新**: 棋盘状态（包括评估分数和 Zobrist 哈希值）随着每一步棋进行增量更新，避免了从头开始的昂贵计算。 |
| **Performance** | **Pre-computed Attack Tables**: Move generation for non-sliding pieces is accelerated using pre-computed attack tables, a standard optimization in high-performance engines. | **预计算攻击表**: 使用预先计算的攻击表来加速非滑动棋子（如马、相）的走法生成，这是高性能引擎的标准优化。 |

---

## Project Structure (项目结构)

-   **/src**: Contains all the Rust source code for the Xiangqi engine, organized into modules for clarity (e.g., `engine`, `evaluate`, `bitboard`).
-   **`Cargo.toml`**: The manifest file for the Rust project, defining dependencies and project settings.
-   **`opening_book.bin`**: A binary opening book file used by the engine.

---

## Getting Started (如何开始)

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/hezhaoyun/xiangqi_rust.git
    cd xiangqi_rust
    ```

2.  **Build and Run:**
    Ensure you have the Rust toolchain installed. Then, build and run the project with Cargo. The following command will compile the engine in release mode (optimized) and launch the Text-UI.
    ```bash
    cargo run --release
    ```

---

## Contributing (贡献)

We welcome contributions from the community! Whether you want to fix a bug, add a new feature, or improve the documentation, please feel free to open an issue or submit a pull request.

欢迎社区的贡献！无论是修复 Bug、添加新功能还是改进文档，都欢迎您提交 Issue 或 Pull Request。

---

## License (许可)

This project is licensed under the MIT License.

该项目采用 MIT 许可协议。
