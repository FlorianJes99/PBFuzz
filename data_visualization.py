# /// script
# requires-python = ">=3.14"
# dependencies = [
#     "matplotlib>=3.10.8",
#     "pandas>=3.0.1",
# ]
# ///


import glob
import sys
from matplotlib import patches
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

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
    paths = glob.glob("out/fuzzing_run_PBS_*/data/orchestrator_data.csv")
    x = []
    y = []
    color_list = []
    xy = (0.3, 300)
    width = 0.2
    height = 500
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
        "duration_iteration_ms",
        "problem_generation_ms",
        "problem_solving_duration_roundingsat_ms",
        "problem_solving_duration_scip_ms",
        "problem_parsing_duration_ms",
    ]

    crop_max = 2000

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
    col_labels = ["iteration", "pb generation", "roundingsat", "scip", "parsing"]
    data_sat = [sat_df[col].dropna() for col in columns]
    axs[0].violinplot(data_sat)
    axs[0].set_title(f"Satisfied ({len(sat_df)} samples)")
    axs[0].set_xticks(range(1, len(columns) + 1))
    axs[0].set_xticklabels(col_labels, rotation=45)

    # Plot for unsat_df
    data_unsat = [unsat_df[col].dropna() for col in columns]
    axs[1].violinplot(data_unsat)
    axs[1].set_title(f"Unsatisfied ({len(unsat_df)} samples)")
    axs[1].set_xticks(range(1, len(columns) + 1))
    axs[1].set_xticklabels(col_labels, rotation=45)

    plt.ylabel("ms")
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

    fig, axs = plt.subplots(nrows=1, ncols=2)

    conflicts = df["conflicts"]
    colors = {
        "sat": "green",
        "unsat": "red",
        "timeout": "blue",
        "error": "yellow",
        "bug": "black",
    }

    color_list = [colors[res] for res in df["result"]]
    roundingsat_ms = df["problem_solving_duration_roundingsat_ms"]

    # roundingsat scatter
    axs[0].scatter(x=roundingsat_ms, y=conflicts, c=color_list)

    # scip scatter
    scip_ms = df["problem_solving_duration_scip_ms"]
    axs[1].scatter(x=scip_ms, y=conflicts, c=color_list)

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
    fig.legend(handles=legend_handles, loc="lower center", ncol=len(colors))
    plt.tight_layout()
    if save:
        plt.savefig("plots/conflict_vs_solving_time.pdf", format="pdf")
    if show:
        plt.show()


def roundingsat_vs_scip_scatter(df: pd.DataFrame, show: bool = True, save: bool = False):
    roundingsat = df["problem_solving_duration_roundingsat_ms"]
    scip = df["problem_solving_duration_scip_ms"]
    fig = plt.figure()
    ax = fig.add_subplot()
    ax.scatter(x=roundingsat, y=scip)
    ax.set_xlabel("roundingsat solving time")
    ax.set_ylabel("scip solving time")
    ax.set_title("Roundingsat vs Scip solving time")
    ax.plot([0, roundingsat.max()], [0, roundingsat.max()], color="red", linestyle="--", linewidth=1, label="equal performance")
    ax.legend()
    if save:
        plt.savefig("plots/roundingsat_vs_scip_scatter.pdf", format="pdf")
    if show:
        plt.show()



def main() -> None:
    # plot_configs()
    csv_path = sys.argv[1]
    df = pd.read_csv(csv_path)
    # plot_result_distribution(df)
    plot_violins_unsat_sat(df, crop=True)
    scatter_conflicts_solving_time(df)
    roundingsat_vs_scip_scatter(df)


# Utility function
def get_sat_df(df: pd.DataFrame) -> pd.DataFrame:
    return df[df["result"] == "sat"]


def get_unsat_df(df: pd.DataFrame) -> pd.DataFrame:
    return df[df["result"] == "unsat"]


if __name__ == "__main__":
    main()
