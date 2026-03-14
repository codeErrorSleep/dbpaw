export type SelectionRangeLike = {
  from: number;
  to: number;
};

export function collectSelectedSql(params: {
  ranges: readonly SelectionRangeLike[];
  sliceDoc: (from: number, to: number) => string;
  fullDoc: () => string;
}): string {
  const { ranges, sliceDoc, fullDoc } = params;
  const selectedSql = ranges
    .map((range) => sliceDoc(range.from, range.to))
    .filter((text) => text.trim().length > 0)
    .join("\n");

  return selectedSql || fullDoc();
}
