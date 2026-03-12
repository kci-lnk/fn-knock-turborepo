type MappingLike = {
  path: string;
  target: string;
};

type SingleValidationOptions = {
  ignorePath?: string | null;
  ignoreTarget?: string | null;
};

const normalize = (value: string) => value.trim();

const uniqueInOrder = (values: string[]) => Array.from(new Set(values));

const findInternalDuplicates = (values: string[]) => {
  const seen = new Set<string>();
  const duplicates: string[] = [];
  for (const value of values) {
    if (seen.has(value)) {
      duplicates.push(value);
      continue;
    }
    seen.add(value);
  }
  return uniqueInOrder(duplicates);
};

export const validateSingleMappingDuplicates = (
  existing: MappingLike[],
  candidate: MappingLike,
  options: SingleValidationOptions = {},
) => {
  const candidatePath = normalize(candidate.path);
  const candidateTarget = normalize(candidate.target);
  const ignorePath = options.ignorePath ? normalize(options.ignorePath) : null;
  const ignoreTarget = options.ignoreTarget ? normalize(options.ignoreTarget) : null;

  const duplicatePath = existing.some((item) => {
    const path = normalize(item.path);
    if (ignorePath && path === ignorePath) return false;
    return path === candidatePath;
  });

  const duplicateTarget = existing.some((item) => {
    const target = normalize(item.target);
    if (ignoreTarget && target === ignoreTarget) return false;
    return target === candidateTarget;
  });

  return {
    duplicatePath,
    duplicateTarget,
  };
};

export const validateBatchMappingDuplicates = (
  existing: MappingLike[],
  candidates: MappingLike[],
) => {
  const existingPathSet = new Set(existing.map((item) => normalize(item.path)));
  const existingTargetSet = new Set(existing.map((item) => normalize(item.target)));

  const candidatePaths = candidates.map((item) => normalize(item.path)).filter(Boolean);
  const candidateTargets = candidates.map((item) => normalize(item.target)).filter(Boolean);

  const duplicateExistingPaths = candidatePaths.filter((path) => existingPathSet.has(path));
  const duplicateExistingTargets = candidateTargets.filter((target) => existingTargetSet.has(target));

  const duplicateInternalPaths = findInternalDuplicates(candidatePaths);
  const duplicateInternalTargets = findInternalDuplicates(candidateTargets);

  return {
    duplicatePaths: uniqueInOrder([...duplicateExistingPaths, ...duplicateInternalPaths]),
    duplicateTargets: uniqueInOrder([...duplicateExistingTargets, ...duplicateInternalTargets]),
  };
};
