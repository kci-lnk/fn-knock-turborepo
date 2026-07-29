export const privilegedNavigationVisibility = ({
  canUseSshSecurity,
  sshSecurityEnabled,
  canUseTerminal,
  terminalEnabled,
}: {
  canUseSshSecurity: boolean;
  sshSecurityEnabled: boolean;
  canUseTerminal: boolean;
  terminalEnabled: boolean;
}) => ({
  sshSecurity: canUseSshSecurity && sshSecurityEnabled,
  terminal: canUseTerminal && terminalEnabled,
});

export const smartConnectFeatureEntryVisible = ({
  isFpkLiteDeployment,
  isDockerDeployment,
  isOpenWrtDeployment,
  isSynologyDeployment,
}: {
  isFpkLiteDeployment: boolean;
  isDockerDeployment: boolean;
  isOpenWrtDeployment: boolean;
  isSynologyDeployment: boolean;
}) =>
  !isFpkLiteDeployment &&
  !isDockerDeployment &&
  !isOpenWrtDeployment &&
  !isSynologyDeployment;
