export type DetailFieldItem = {
  key: string;
  label: string;
  value: string | number | boolean;
};

type DetailFieldDefinition<T extends Record<string, any>> = {
  key: keyof T | string;
  label: string;
  format?: (value: any, record: T) => string | number | boolean;
  includeWhenUndefined?: boolean;
};

const DEFAULT_EMPTY_VALUE = '-';

type BuildDetailFieldsOptions = {
  format?: (key: string, value: any) => string | number | boolean;
  emptyValue?: string | number | boolean;
};

export const buildDetailFields = <T extends Record<string, any>>(
  record: T | null | undefined,
  definitions: ReadonlyArray<DetailFieldDefinition<T>>,
  options: BuildDetailFieldsOptions = {},
): DetailFieldItem[] => {
  if (!record) return [];

  return definitions
    .filter((definition) => {
      if (definition.includeWhenUndefined) return true;
      const value = record[definition.key as keyof T];
      return value !== undefined;
    })
    .map((definition) => {
      const raw = record[definition.key as keyof T];
      const formatted = definition.format
        ? definition.format(raw, record)
        : options.format
          ? options.format(String(definition.key), raw)
          : raw;
      const value = formatted ?? options.emptyValue ?? DEFAULT_EMPTY_VALUE;
      return {
        key: String(definition.key),
        label: definition.label,
        value,
      };
    });
};
