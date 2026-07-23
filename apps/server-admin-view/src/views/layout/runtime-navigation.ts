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
