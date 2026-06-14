import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';

type DefaultRouteAction = 'clear' | 'set';

export const useDefaultRouteConfirm = (defaultSystemPort: number) => {
  const { t } = useI18n();
  const open = ref(false);
  const pendingPath = ref<string | null>(null);
  const pendingAction = ref<DefaultRouteAction | null>(null);
  const pendingTargetPort = ref<number | null>(null);

  const showDefaultRouteFnosHint = computed(() => pendingTargetPort.value === defaultSystemPort);
  const dialogTitle = computed(() =>
    pendingAction.value === 'clear'
      ? t('shared.defaultRouteConfirm.clearTitle')
      : t('shared.defaultRouteConfirm.setTitle'),
  );
  const dialogDescription = computed(() => {
    if (pendingAction.value === 'clear') {
      return showDefaultRouteFnosHint.value
        ? t('shared.defaultRouteConfirm.clearFnosDescription', { port: defaultSystemPort })
        : t('shared.defaultRouteConfirm.clearDescription');
    }
    return t('shared.defaultRouteConfirm.setDescription', { port: defaultSystemPort });
  });

  const queue = (path: string, action: DefaultRouteAction, targetPort: number | null) => {
    pendingPath.value = path;
    pendingAction.value = action;
    pendingTargetPort.value = targetPort;
    open.value = true;
  };

  const reset = () => {
    open.value = false;
    pendingPath.value = null;
    pendingAction.value = null;
    pendingTargetPort.value = null;
  };

  return {
    open,
    pendingPath,
    showDefaultRouteFnosHint,
    dialogTitle,
    dialogDescription,
    queue,
    reset,
  };
};
