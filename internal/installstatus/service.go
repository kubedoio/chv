package installstatus

import "github.com/chv/chv/internal/models"

func Evaluate(status *models.InstallStatus, drift bool, missingDirs bool) models.InstallState {
	switch {
	case drift:
		return models.InstallStateDriftDetected
	case !status.CloudHypervisorFound || !status.CloudInitSupported:
		return models.InstallStateMissingPrerequisites
	case missingDirs || !status.BridgeExists || !status.LocaldiskReady:
		return models.InstallStateBootstrapRequired
	default:
		return models.InstallStateReady
	}
}
