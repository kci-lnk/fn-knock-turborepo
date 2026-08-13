export interface CursorPaginationLabels {
  ariaLabel: string;
  canLoadOlder: string;
  firstPage: string;
  lastPage: string;
  nextPage: string;
  pageSize: string;
  pageSizeOption: (count: string) => string;
  previousPage: string;
}
