import { computed, reactive, ref } from 'vue';
import type { ProxyMappingFields } from '@admin-shared/utils/proxyMapping';

type ProxyMappingFormValue = ProxyMappingFields & {
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
};

type ProxyMappingEditable = ProxyMappingFields & Partial<Pick<ProxyMappingFormValue, 'rewrite_html' | 'use_auth' | 'use_root_mode' | 'strip_path'>>;

export const useProxyMappingDialogForm = <T extends ProxyMappingEditable>(
  defaultFlags: Pick<ProxyMappingFormValue, 'rewrite_html' | 'use_auth' | 'use_root_mode' | 'strip_path'>,
) => {
  const open = ref(false);
  const isEditing = ref(false);
  const editingOriginal = ref<T | null>(null);
  const form = reactive<ProxyMappingFormValue>({
    path: '',
    target: '',
    ...defaultFlags,
  });

  const isValid = computed(() => form.path.trim() !== '' && form.target.trim() !== '');

  const resetForm = () => {
    form.path = '';
    form.target = '';
    form.rewrite_html = defaultFlags.rewrite_html;
    form.use_auth = defaultFlags.use_auth;
    form.use_root_mode = defaultFlags.use_root_mode;
    form.strip_path = defaultFlags.strip_path;
  };

  const openAdd = () => {
    isEditing.value = false;
    editingOriginal.value = null;
    resetForm();
    open.value = true;
  };

  const openEdit = (mapping: T) => {
    isEditing.value = true;
    editingOriginal.value = mapping;
    form.path = mapping.path;
    form.target = mapping.target;
    form.rewrite_html = !!mapping.rewrite_html;
    form.use_auth = !!mapping.use_auth;
    form.use_root_mode = !!mapping.use_root_mode;
    form.strip_path = !!mapping.strip_path;
    open.value = true;
  };

  const close = (reset = false) => {
    open.value = false;
    if (reset) {
      isEditing.value = false;
      editingOriginal.value = null;
      resetForm();
    }
  };

  return {
    open,
    isEditing,
    editingOriginal,
    form,
    isValid,
    openAdd,
    openEdit,
    close,
    resetForm,
  };
};
