import { toast } from '@admin-shared/utils/toast';
import { extractErrorMessage } from '@admin-shared/composables/useAsyncAction';

type Translate = (key: string, named?: Record<string, unknown>) => string;

export const createReverseProxyMessages = (t: Translate) => ({
  get unknownError() {
    return t('admin.reverseProxy.feedback.unknownError');
  },
  get networkError() {
    return t('admin.reverseProxy.feedback.networkError');
  },
  get syncFailed() {
    return t('admin.reverseProxy.feedback.syncFailed');
  },
  get deleteFailed() {
    return t('admin.reverseProxy.feedback.deleteFailed');
  },
  get deleteSuccess() {
    return t('admin.reverseProxy.feedback.deleteSuccess');
  },
  get saveFailed() {
    return t('admin.reverseProxy.feedback.saveFailed');
  },
  get createSuccess() {
    return t('admin.reverseProxy.feedback.createSuccess');
  },
  get updateSuccess() {
    return t('admin.reverseProxy.feedback.updateSuccess');
  },
  get defaultRouteUpdateFailed() {
    return t('admin.reverseProxy.feedback.defaultRouteUpdateFailed');
  },
  get scanFailed() {
    return t('admin.reverseProxy.feedback.scanFailed');
  },
  duplicatePath: (path: string) => t('admin.reverseProxy.feedback.duplicatePath', { path }),
  duplicateTarget: (target: string) =>
    t('admin.reverseProxy.feedback.duplicateTarget', { target }),
  duplicateItems: (label: string, values: string[]) =>
    t('admin.reverseProxy.feedback.duplicateItems', {
      label,
      values: values.join(t('admin.reverseProxy.feedback.listSeparator')),
    }),
  syncSuccess: (count: number) => t('admin.reverseProxy.feedback.syncSuccess', { count }),
  discoverSaveSuccess: (count: number) =>
    t('admin.reverseProxy.feedback.discoverSaveSuccess', { count }),
});

export const showReverseProxyActionError = (title: string, error: unknown, fallback: string) => {
  toast.error(`${title}: ${extractErrorMessage(error, fallback)}`);
};

export const showReverseProxyDuplicateItemsError = (message: string) => {
  toast.error(message);
};

export const showReverseProxyBooleanResultToast = (
  result: { success?: boolean; message?: string },
  options: { successText: string; errorText: string; unknownErrorText: string },
) => {
  if (result.success) {
    toast.success(result.message || options.successText);
    return true;
  }

  toast.error(`${options.errorText}: ${result.message || options.unknownErrorText}`);
  return false;
};
