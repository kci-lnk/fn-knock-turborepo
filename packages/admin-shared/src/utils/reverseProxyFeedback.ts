import { toast } from '@admin-shared/utils/toast';
import { extractErrorMessage } from '@admin-shared/composables/useAsyncAction';

export const REVERSE_PROXY_MESSAGES = {
  unknownError: '未知错误',
  networkError: '网络错误',
  syncFailed: '同步失败',
  deleteFailed: '删除失败',
  deleteSuccess: '代理映射已删除',
  saveFailed: '保存失败',
  createSuccess: '代理映射已添加',
  updateSuccess: '代理映射已更新',
  defaultRouteUpdateFailed: '默认路由更新失败',
  scanFailed: '端口扫描失败',
  duplicatePath: (path: string) => `路径 ${path} 已存在，不能重复添加`,
  duplicateTarget: (target: string) => `目标地址 ${target} 已存在，不能重复添加`,
  duplicateItems: (label: string, values: string[]) => `以下${label}已存在或重复：${values.join('、')}`,
  syncSuccess: (count: number) => `成功同步 ${count} 条路由规则`,
  discoverSaveSuccess: (count: number) => `成功添加/更新 ${count} 条代理映射`,
} as const;

export const showReverseProxyActionError = (title: string, error: unknown, fallback: string) => {
  toast.error(`${title}: ${extractErrorMessage(error, fallback)}`);
};

export const showReverseProxyDuplicateItemsError = (label: string, values: string[]) => {
  toast.error(REVERSE_PROXY_MESSAGES.duplicateItems(label, values));
};

export const showReverseProxyBooleanResultToast = (
  result: { success?: boolean; message?: string },
  options: { successText: string; errorText: string },
) => {
  if (result.success) {
    toast.success(result.message || options.successText);
    return true;
  }

  toast.error(`${options.errorText}: ${result.message || REVERSE_PROXY_MESSAGES.unknownError}`);
  return false;
};
