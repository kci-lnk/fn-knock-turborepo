export const privilegedNavigationVisibility = ({
  canUseSshSecurity,
  sshSecurityEnabled,
}: {
  canUseSshSecurity: boolean;
  sshSecurityEnabled: boolean;
}) => ({
  sshSecurity: canUseSshSecurity && sshSecurityEnabled,
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
