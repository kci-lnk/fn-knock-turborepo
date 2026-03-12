import { computed, ref } from 'vue';

type DefaultRouteAction = 'clear' | 'set';

export const useDefaultRouteConfirm = (defaultSystemPort: number) => {
  const open = ref(false);
  const pendingPath = ref<string | null>(null);
  const pendingAction = ref<DefaultRouteAction | null>(null);
  const pendingTargetPort = ref<number | null>(null);

  const showDefaultRouteFnosHint = computed(() => pendingTargetPort.value === defaultSystemPort);
  const dialogTitle = computed(() =>
    pendingAction.value === 'clear' ? '确认清除默认路由？' : '确认设置默认路由？',
  );
  const dialogDescription = computed(() => {
    if (pendingAction.value === 'clear') {
      return showDefaultRouteFnosHint.value
        ? '你正在清除 5666 端口服务的默认路由，可能影响飞牛 OS 的默认入口访问。'
        : '清除后将不再有默认路由，未命中路径的请求可能无法按预期转发。';
    }
    return '当前默认路由为 5666 端口服务，切换到其它路由后可能影响飞牛 OS 的默认入口。';
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
