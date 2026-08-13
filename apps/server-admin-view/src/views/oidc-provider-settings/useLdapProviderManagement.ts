import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@frontend-core/errors/extractErrorMessage";
import { ConfigAPI } from "@/lib/api/config";
import type {
  LdapProviderCatalogItem,
  LdapProviderType,
  LdapProviderView,
} from "@/types";

export interface LdapProviderForm {
  baseDn: string;
  bindMode: "search" | "direct";
  caPem: string;
  directBindTemplate: string;
  displayNameAttribute: string;
  emailAttribute: string;
  enabled: boolean;
  name: string;
  servers: string;
  serviceBindDn: string;
  serviceBindPassword: string;
  subjectAttribute: string;
  transport: "ldaps" | "starttls";
  type: LdapProviderType;
  userFilter: string;
  usernameAttribute: string;
}

const readText = (config: Record<string, unknown>, key: string) => {
  const value = config[key];
  return typeof value === "string" ? value : "";
};
export const readLdapServers = (config: Record<string, unknown>) =>
  Array.isArray(config.servers)
    ? config.servers.map(String).join("\n")
    : readText(config, "servers");

export const useLdapProviderManagement = () => {
  const { t } = useI18n();
  const catalog = ref<LdapProviderCatalogItem[]>([]);
  const providers = ref<LdapProviderView[]>([]);
  const isLoading = ref(false);
  const isSaving = ref(false);
  const mutatingId = ref("");
  const showDialog = ref(false);
  const editingId = ref("");
  const showTestCredentialsDialog = ref(false);
  const testingProvider = ref<LdapProviderView | null>(null);
  const testUsername = ref("");
  const testPassword = ref("");
  const form = reactive<LdapProviderForm>({
    type: "openldap",
    name: "OpenLDAP",
    enabled: true,
    servers: "ldaps://ldap.example.com:636",
    transport: "ldaps",
    bindMode: "search",
    baseDn: "",
    userFilter: "(&(objectClass=person)(uid={username}))",
    serviceBindDn: "",
    serviceBindPassword: "",
    directBindTemplate: "uid={username},ou=people,dc=example,dc=com",
    subjectAttribute: "entryUUID",
    usernameAttribute: "uid",
    displayNameAttribute: "cn",
    emailAttribute: "mail",
    caPem: "",
  });

  const applyPreset = (type: LdapProviderType) => {
    form.type = type;
    const preset = catalog.value.find((item) => item.type === type);
    if (!preset) return;
    form.name = preset.label;
    form.transport = preset.defaults.transport;
    form.bindMode = preset.defaults.bind_mode;
    form.userFilter = preset.defaults.user_filter;
    form.subjectAttribute = preset.defaults.subject_attribute;
    form.usernameAttribute = preset.defaults.username_attribute;
    form.displayNameAttribute = preset.defaults.display_name_attribute;
    form.emailAttribute = preset.defaults.email_attribute;
    form.directBindTemplate = type === "active_directory" ? "{username}" : "";
  };
  const resetForm = () => {
    editingId.value = "";
    form.baseDn = "";
    form.servers = "ldaps://ldap.example.com:636";
    form.serviceBindDn = "";
    form.serviceBindPassword = "";
    form.caPem = "";
    form.enabled = true;
    applyPreset(catalog.value[0]?.type || "openldap");
  };
  const load = async () => {
    isLoading.value = true;
    try {
      const [definitions, items] = await Promise.all([
        ConfigAPI.getLdapProviderCatalog(),
        ConfigAPI.getLdapProviders(),
      ]);
      catalog.value = definitions;
      providers.value = items;
    } catch (error) {
      toast.error(
        extractErrorMessage(error, t("admin.ldapProviders.loadFailed")),
      );
    } finally {
      isLoading.value = false;
    }
  };
  const openCreate = () => {
    resetForm();
    showDialog.value = true;
  };
  const openEdit = (provider: LdapProviderView) => {
    const config = provider.connection_config || {};
    editingId.value = provider.id;
    form.type = provider.type;
    form.name = provider.name;
    form.enabled = provider.enabled;
    form.servers = readLdapServers(config);
    form.transport =
      readText(config, "transport") === "starttls" ? "starttls" : "ldaps";
    form.bindMode =
      readText(config, "bind_mode") === "direct" ? "direct" : "search";
    form.baseDn = readText(config, "base_dn");
    form.userFilter = readText(config, "user_filter");
    form.serviceBindDn = readText(config, "service_bind_dn");
    form.serviceBindPassword = "";
    form.directBindTemplate = readText(config, "direct_bind_template");
    form.subjectAttribute = readText(config, "subject_attribute");
    form.usernameAttribute = readText(config, "username_attribute");
    form.displayNameAttribute = readText(config, "display_name_attribute");
    form.emailAttribute = readText(config, "email_attribute");
    form.caPem = readText(config, "ca_pem");
    showDialog.value = true;
  };
  const setEditorDialogOpen = (open: boolean) => {
    showDialog.value = open;
    if (!open) form.serviceBindPassword = "";
  };
  const payload = () => ({
    name: form.name.trim(),
    type: form.type,
    enabled: form.enabled,
    connection_config: {
      servers: form.servers
        .split(/\r?\n|,/u)
        .map((item) => item.trim())
        .filter(Boolean),
      transport: form.transport,
      bind_mode: form.bindMode,
      base_dn: form.baseDn.trim(),
      user_filter: form.userFilter.trim(),
      service_bind_dn: form.serviceBindDn.trim(),
      service_bind_password: form.serviceBindPassword,
      direct_bind_template: form.directBindTemplate.trim(),
      subject_attribute: form.subjectAttribute.trim(),
      username_attribute: form.usernameAttribute.trim(),
      display_name_attribute: form.displayNameAttribute.trim(),
      email_attribute: form.emailAttribute.trim(),
      ca_pem: form.caPem.trim(),
    },
  });
  const save = async () => {
    isSaving.value = true;
    try {
      if (editingId.value) {
        await ConfigAPI.updateLdapProvider(editingId.value, payload());
      } else {
        await ConfigAPI.createLdapProvider(payload());
      }
      toast.success(t("admin.ldapProviders.saved"));
      showDialog.value = false;
      await load();
    } catch (error) {
      toast.error(
        extractErrorMessage(error, t("admin.ldapProviders.saveFailed")),
      );
    } finally {
      isSaving.value = false;
      form.serviceBindPassword = "";
    }
  };
  const runProviderTest = async (
    provider: LdapProviderView,
    credentials?: { username: string; password: string },
  ) => {
    mutatingId.value = provider.id;
    try {
      const result = await ConfigAPI.testLdapProvider(provider.id, credentials);
      if (!result.success) throw new Error(result.message);
      toast.success(result.message || t("admin.ldapProviders.testSucceeded"));
      await load();
    } catch (error) {
      toast.error(
        extractErrorMessage(error, t("admin.ldapProviders.testFailed")),
      );
    } finally {
      mutatingId.value = "";
    }
  };
  const testProvider = async (provider: LdapProviderView) => {
    if (readText(provider.connection_config, "bind_mode") === "direct") {
      testingProvider.value = provider;
      testUsername.value = "";
      testPassword.value = "";
      showTestCredentialsDialog.value = true;
      return;
    }
    await runProviderTest(provider);
  };
  const runDirectProviderTest = async () => {
    const provider = testingProvider.value;
    if (!provider || !testUsername.value.trim() || !testPassword.value) {
      toast.error(t("admin.ldapProviders.testCredentialsRequired"));
      return;
    }
    showTestCredentialsDialog.value = false;
    await runProviderTest(provider, {
      username: testUsername.value.trim(),
      password: testPassword.value,
    });
    testPassword.value = "";
    testingProvider.value = null;
  };
  const setTestCredentialsDialogOpen = (open: boolean) => {
    showTestCredentialsDialog.value = open;
    if (!open) {
      testPassword.value = "";
      testingProvider.value = null;
    }
  };
  const removeProvider = async (id: string) => {
    mutatingId.value = id;
    try {
      await ConfigAPI.deleteLdapProvider(id);
      toast.success(t("admin.ldapProviders.deleted"));
      await load();
    } catch (error) {
      toast.error(
        extractErrorMessage(error, t("admin.ldapProviders.deleteFailed")),
      );
    } finally {
      mutatingId.value = "";
    }
  };

  onMounted(load);
  return {
    applyPreset,
    catalog,
    editingId,
    form,
    isLoading,
    isSaving,
    mutatingId,
    openCreate,
    openEdit,
    providers,
    readServers: readLdapServers,
    removeProvider,
    runDirectProviderTest,
    save,
    setEditorDialogOpen,
    setTestCredentialsDialogOpen,
    showDialog,
    showTestCredentialsDialog,
    testPassword,
    testProvider,
    testUsername,
  };
};
