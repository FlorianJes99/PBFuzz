# /// script
# requires-python = ">=3.14"
# dependencies = [
#     "matplotlib>=3.10.8",
#     "numpy>=2.4.3",
#     "pandas>=3.0.1",
# ]
# ///


import glob
import random
import subprocess
import sys
import time
from matplotlib import patches
from matplotlib.axes import Axes
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import matplotlib.lines as mlines

# pub iteration: u32,
# pub result: &'static str,
# pub duration_iteration_ms: f32,
# pub problem_generation_ms: f32,
# pub problem_solving_duration_roundingsat_ms: f32,
# pub problem_solving_duration_scip_ms: f32,
# pub problem_parsing_duration_ms: f32,
# pub conflicts: u32,
# pub seed: u64,


def plot_configs(show: bool = True, save: bool = False):
    """Plot different configurations for fuzzer to determine the best ones for our use case."""
    paths = glob.glob("out/fuzzing_run_*/data/orchestrator_data.csv")
    x = []
    y = []
    color_list = []
    xy = (0.3, 300)
    width = 0.2
    height = 700
    candidates = []

    def is_in_bound(x_d: float, y_d: int) -> bool:
        return xy[0] <= x_d <= xy[0] + width and xy[1] <= y_d <= xy[1] + height

    for path in paths:
        df = pd.read_csv(path)
        df_sat = df[df["result"] == "sat"]
        df_unsat = df[df["result"] == "unsat"]
        amount_sat = len(df_sat)
        amount_unsat = len(df_unsat)
        duration_iteration_mean = (
            df_sat["duration_iteration_ms"].mean()
            + df_unsat["duration_iteration_ms"].mean()
        ) / 2
        sat_ratio = amount_sat / (amount_sat + amount_unsat)
        x.append(sat_ratio)
        y.append(duration_iteration_mean)
        if is_in_bound(sat_ratio, duration_iteration_mean):
            color_list.append("green")
            candidates.append(path)
        else:
            color_list.append("red")
    fig = plt.figure()
    ax = fig.add_subplot()
    ax.scatter(x=x, y=y, c=color_list)
    ax.add_patch(
        patches.Rectangle(
            xy=xy, width=width, height=height, linewidth=1, color="red", fill=False
        )
    )
    legend_handles = [
        mpatches.Patch(color="green", label="usable config"),
        mpatches.Patch(color="red", label="not usable config"),
    ]
    fig.legend(handles=legend_handles)
    plt.xlabel("SAT ratio")
    plt.ylabel("Iteration duration in ms (without timeouts)")
    plt.title(label="Configurations for fuzzer (1000 iterations per run)")
    if save:
        plt.savefig("plots/configurations.pdf", format="pdf")
    if show:
        plt.show()
    print("Candidates:")
    print("\n".join(candidates))


def plot_result_distribution(df: pd.DataFrame, show: bool = True, save: bool = False):
    counts = df["result"].value_counts()
    counts.plot(kind="bar", color="blue")
    plt.xlabel("Result")
    plt.ylabel("Amount")
    plt.title("Result distribution")
    if save:
        plt.savefig("plots/result_distribution.pdf", format="pdf")
    if show:
        plt.show()


def plot_violins_unsat_sat(
    df: pd.DataFrame, show: bool = True, save: bool = False, crop: bool = False
):
    columns = [
        "problem_generation_ms",
        "problem_solving_duration_roundingsat_ms",
        "problem_solving_duration_scip_ms",
        "problem_parsing_duration_ms",
    ]

    crop_max = 7000

    def crop_vals(df: pd.DataFrame) -> pd.DataFrame:
        for col in columns:
            df = df[df[col] <= crop_max]
        return df

    if crop:
        df = crop_vals(df)
    sat_df = get_sat_df(df)
    unsat_df = get_unsat_df(df)
    fig, axs = plt.subplots(nrows=1, ncols=2)

    # Plot for sat_df
    col_labels = ["pb generation", "roundingsat", "scip", "parsing"]
    data_sat = [sat_df[col].dropna() for col in columns]
    axs[0].violinplot(data_sat, showmedians=True)
    axs[0].set_title(f"Satisfied ({len(sat_df)} samples)")
    axs[0].set_xticks(range(1, len(columns) + 1))
    axs[0].set_xticklabels(col_labels, rotation=45)
    axs[0].set_yscale("log")

    # Plot for unsat_df
    data_unsat = [unsat_df[col].dropna() for col in columns]
    axs[1].violinplot(data_unsat, showmedians=True)
    axs[1].set_title(f"Unsatisfied ({len(unsat_df)} samples)")
    axs[1].set_xticks(range(1, len(columns) + 1))
    axs[1].set_xticklabels(col_labels, rotation=45)
    axs[1].set_yscale("log")

    plt.ylabel("log scale - duration (ms)")
    plt.tight_layout()
    if save:
        path = (
            f"plots/violins_sat_unsat_cropped_{crop_max}ms.pdf"
            if crop
            else "plots/violins_sat_unsat.pdf"
        )
        plt.savefig(path, format="pdf")
    if show:
        plt.show()


def scatter_conflicts_solving_time(
    df: pd.DataFrame, show: bool = True, save: bool = False
):

    df = df[df["result"].isin(["sat", "unsat"])]
    axs: list[Axes]
    fig, axs = plt.subplots(nrows=1, ncols=2)

    conflicts = df["conflicts"]
    colors = {
        "sat": "green",
        "unsat": "red",
    }

    color_list = [colors[res] for res in df["result"]]
    roundingsat_ms = df["problem_solving_duration_roundingsat_ms"]

    # roundingsat scatter
    axs[0].scatter(x=roundingsat_ms, y=conflicts, c=color_list, alpha=0.3)

    # scip scatter
    scip_ms = df["problem_solving_duration_scip_ms"]
    axs[1].scatter(x=scip_ms, y=conflicts, c=color_list, alpha=0.3)

    # Add titles and labels
    axs[0].set_title("Roundingsat: Conflicts vs Solving Time")
    axs[0].set_xlabel("Solving Time (ms)")
    axs[0].set_ylabel("Conflicts")

    axs[1].set_title("SCIP: Conflicts vs Solving Time")
    axs[1].set_xlabel("Solving Time (ms)")
    axs[1].set_ylabel("Conflicts")
    legend_handles = [
        mpatches.Patch(color=color, label=result) for result, color in colors.items()
    ]
    fig.legend(handles=legend_handles, loc="lower center", ncol=len(colors), fontsize=7, markerscale=0.7)
    plt.tight_layout()
    if save:
        plt.savefig("plots/conflict_vs_solving_time.png", format="png", dpi=300, bbox_inches="tight")
    if show:
        plt.show()


def roundingsat_vs_scip_scatter(
    df: pd.DataFrame, show: bool = True, save: bool = False
):
    colors = {
        "sat": "green",
        "unsat": "red",
        "timeout": "blue",
        "error": "yellow",
        "bug": "black",
    }

    color_list = df["result"].map(lambda x: colors[x])
    roundingsat = df["problem_solving_duration_roundingsat_ms"]
    scip = df["problem_solving_duration_scip_ms"]
    fig = plt.figure()
    ax = fig.add_subplot()
    ax.scatter(x=roundingsat, y=scip, c=color_list, alpha=0.3, s=2)
    ax.set_xlabel("roundingsat solving time")
    ax.set_ylabel("scip solving time")
    ax.set_title("Roundingsat vs Scip solving time")
    legend_handles = [
        mpatches.Patch(color=color, label=result) for result, color in colors.items()
    ]
    legend_handles.append(
        mlines.Line2D(
            [],
            [],
            color="black",
            linestyle="--",
            linewidth=1,
            label="equal performance",
        )
    )
    ax.plot(
        [0, roundingsat.max()],
        [0, roundingsat.max()],
        color="black",
        linestyle="--",
        linewidth=1,
        label="equal performance",
    )
    ax.legend(handles=legend_handles, loc="upper left", ncol=3, fontsize=7, markerscale=0.7)
    ax.set_xlim(left=0)
    ax.set_ylim(bottom=0)
    plt.tight_layout()
    if save:
        plt.savefig("plots/roundingsat_vs_scip_scatter.png", format="png", dpi=300, bbox_inches="tight")
    if show:
        plt.show()


def fuzzer_bench(
    fuzz_bin: str, num_runs: int = 1000, show: bool = True, save: bool = False
):
    times = []
    for i in range(num_runs):
        seed = random.randint(0, 2**32)

        start = time.perf_counter()
        subprocess.run(
            [
                fuzz_bin,
                "--seed",
                str(seed),
                "--settings-file",
                "configuration/large_problems_settings.ron",
                "--output-path",
                "out/fuzzer_bench/test.opb",
            ]
        )
        elapsed = time.perf_counter() - start
        elapsed = elapsed * 1000
        times.append(elapsed)
        print(f"{i + 1}/{num_runs}")

    fig = plt.figure()
    ax = fig.add_subplot()
    ax.violinplot(times, showmedians=True, showextrema=True)
    ax.set_xticks([])
    ax.set_ylabel("wall clock time fuzzer (ms)")
    ax.set_title(f"Benchmarks for fuzzer problem generations ({num_runs} iterations)")
    if save:
        plt.savefig("plots/benchmark_fuzzer.pdf", format="pdf")
    if show:
        plt.show()


def main() -> None:
    # plot_configs()
    save = True
    csv_path = sys.argv[1]
    df = pd.read_csv(csv_path)
    plot_result_distribution(df, save=save)
    plot_violins_unsat_sat(df, save=save)
    scatter_conflicts_solving_time(df, save=save)
    roundingsat_vs_scip_scatter(df, save=save)
    # fuzzer_bench("target/release/fuzzer", num_runs=10000, save=True)


# Utility function
def get_sat_df(df: pd.DataFrame) -> pd.DataFrame:
    return df[df["result"] == "sat"]


def get_unsat_df(df: pd.DataFrame) -> pd.DataFrame:
    return df[df["result"] == "unsat"]


if __name__ == "__main__":
    main()
