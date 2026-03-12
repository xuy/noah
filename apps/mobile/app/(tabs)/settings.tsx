import { useState, useEffect, useRef } from "react";
import {
  View,
  Text,
  TouchableOpacity,
  StyleSheet,
  Alert,
  Platform,
  ScrollView,
  Modal,
  ActivityIndicator,
} from "react-native";
import { CameraView } from "expo-camera";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../../constants/theme";
import { useAuth } from "../_layout";
import {
  parseQrPayload,
  pairWithDesktop,
  getDesktopStatus,
  loadPairing,
  clearPairing,
  type PairingInfo,
  type DesktopStatus,
} from "../../lib/desktop-client";

export default function SettingsTab() {
  const { user, signOut } = useAuth();

  // Pairing state
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [desktopStatus, setDesktopStatus] = useState<DesktopStatus | null>(null);
  const [showScanner, setShowScanner] = useState(false);
  const [isPairing, setIsPairing] = useState(false);
  const scannedRef = useRef(false);

  useEffect(() => {
    loadPairingState();
  }, []);

  // Periodically check desktop status when paired
  useEffect(() => {
    if (!pairing) {
      setDesktopStatus(null);
      return;
    }
    let cancelled = false;
    async function check() {
      try {
        const status = await getDesktopStatus(pairing!);
        if (!cancelled) setDesktopStatus(status);
      } catch {
        if (!cancelled) setDesktopStatus(null);
      }
    }
    check();
    const interval = setInterval(check, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [pairing]);

  async function loadPairingState() {
    const p = await loadPairing();
    setPairing(p);
  }

  async function handleAutoPair() {
    setIsPairing(true);
    try {
      const res = await fetch("http://192.168.86.22:7892/generate-qr");
      if (!res.ok) throw new Error("Desktop not reachable");
      const data = await res.json();
      const qr = parseQrPayload(data.qr_json);
      if (!qr) throw new Error("Invalid QR data from desktop");
      const result = await pairWithDesktop(qr, `${Platform.OS} phone`);
      setPairing(result);
      Alert.alert("Paired!", `Connected to ${result.desktopName}`);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Auto-pair failed";
      Alert.alert("Auto-Pair Failed", msg);
    } finally {
      setIsPairing(false);
    }
  }

  async function handleBarcodeScan(data: string) {
    if (scannedRef.current || isPairing) return;
    scannedRef.current = true;

    const qr = parseQrPayload(data);
    if (!qr) {
      Alert.alert("Invalid QR", "This QR code is not a Noah pairing code.");
      scannedRef.current = false;
      return;
    }

    setIsPairing(true);
    try {
      const result = await pairWithDesktop(qr, `${Platform.OS} phone`);
      setPairing(result);
      setShowScanner(false);
      Alert.alert("Paired!", `Connected to ${result.desktopName}`);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Pairing failed";
      Alert.alert("Pairing Failed", msg);
      scannedRef.current = false;
    } finally {
      setIsPairing(false);
    }
  }

  async function handleUnpair() {
    Alert.alert("Disconnect", "Unpair from desktop?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Disconnect",
        style: "destructive",
        onPress: async () => {
          await clearPairing();
          setPairing(null);
          setDesktopStatus(null);
        },
      },
    ]);
  }

  function handleSignOut() {
    Alert.alert("Sign Out", "Sign out of your Noah account?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Sign Out",
        style: "destructive",
        onPress: signOut,
      },
    ]);
  }

  return (
    <SafeAreaView style={styles.container} edges={["bottom"]}>
      <ScrollView
        contentContainerStyle={styles.scrollContent}
        keyboardShouldPersistTaps="handled"
      >
        {/* Account Section */}
        <View style={styles.section}>
          <Text style={styles.sectionTitle}>Account</Text>
          {user && (
            <View style={styles.accountInfo}>
              <View style={styles.accountRow}>
                <Text style={styles.accountLabel}>Email</Text>
                <Text style={styles.accountValue}>{user.email}</Text>
              </View>
              {user.name && (
                <View style={styles.accountRow}>
                  <Text style={styles.accountLabel}>Name</Text>
                  <Text style={styles.accountValue}>{user.name}</Text>
                </View>
              )}
              <View style={styles.accountRow}>
                <Text style={styles.accountLabel}>Plan</Text>
                <Text style={styles.tierBadge}>{user.subscription_tier}</Text>
              </View>
              {user.expires_at && (
                <View style={styles.accountRow}>
                  <Text style={styles.accountLabel}>Expires</Text>
                  <Text style={styles.accountValue}>
                    {new Date(user.expires_at).toLocaleDateString()}
                  </Text>
                </View>
              )}
            </View>
          )}
          <TouchableOpacity style={styles.dangerButton} onPress={handleSignOut}>
            <Text style={styles.dangerButtonText}>Sign Out</Text>
          </TouchableOpacity>
        </View>

        {/* Desktop Pairing Section */}
        <View style={styles.section}>
          <Text style={styles.sectionTitle}>Desktop Pairing</Text>
          {pairing ? (
            <>
              <View style={styles.statusRow}>
                <View
                  style={[
                    styles.statusDot,
                    { backgroundColor: desktopStatus?.online ? colors.statusActive : colors.accentRed },
                  ]}
                />
                <Text style={styles.statusText}>
                  {desktopStatus?.online
                    ? `Connected to ${pairing.desktopName}`
                    : "Desktop offline"}
                </Text>
              </View>
              {desktopStatus?.online && desktopStatus.pending_approvals > 0 && (
                <Text style={styles.pendingText}>
                  {desktopStatus.pending_approvals} pending approval(s)
                </Text>
              )}
              <TouchableOpacity style={styles.dangerButton} onPress={handleUnpair}>
                <Text style={styles.dangerButtonText}>Disconnect</Text>
              </TouchableOpacity>
            </>
          ) : (
            <>
              <Text style={styles.sectionDescription}>
                Pair with Noah Desktop to send photo analyses for automatic
                diagnosis and remediation on your computer.
              </Text>
              <View style={styles.buttonRow}>
                <TouchableOpacity
                  style={styles.primaryButton}
                  onPress={() => {
                    scannedRef.current = false;
                    setShowScanner(true);
                  }}
                >
                  <Text style={styles.primaryButtonText}>Scan QR</Text>
                </TouchableOpacity>
                <TouchableOpacity
                  style={styles.secondaryButton}
                  onPress={handleAutoPair}
                  disabled={isPairing}
                >
                  {isPairing ? (
                    <ActivityIndicator color={colors.textPrimary} size="small" />
                  ) : (
                    <Text style={styles.secondaryButtonText}>Auto-Pair</Text>
                  )}
                </TouchableOpacity>
              </View>
            </>
          )}
        </View>

        <View style={styles.footer}>
          <Text style={styles.footerText}>Noah v1.0.0</Text>
        </View>
      </ScrollView>

      {/* QR Scanner Modal */}
      <Modal visible={showScanner} animationType="slide">
        <SafeAreaView style={styles.scannerContainer}>
          <View style={styles.scannerHeader}>
            <Text style={styles.scannerTitle}>Scan Pairing QR Code</Text>
            <TouchableOpacity onPress={() => setShowScanner(false)}>
              <Text style={styles.scannerClose}>Cancel</Text>
            </TouchableOpacity>
          </View>
          <Text style={styles.scannerHint}>
            Open Noah Desktop → Settings → Mobile Pairing to show the QR code.
          </Text>
          {isPairing ? (
            <View style={styles.centered}>
              <ActivityIndicator color={colors.accentTeal} size="large" />
              <Text style={styles.pairingText}>Pairing...</Text>
            </View>
          ) : (
            <CameraView
              style={styles.scanner}
              facing="back"
              barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
              onBarcodeScanned={(result) => handleBarcodeScan(result.data)}
            />
          )}
        </SafeAreaView>
      </Modal>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bgPrimary,
  },
  centered: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
  },
  scrollContent: {
    padding: 20,
    gap: 16,
    flexGrow: 1,
  },
  section: {
    backgroundColor: colors.bgSecondary,
    borderRadius: 16,
    padding: 20,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
  },
  sectionTitle: {
    fontSize: 20,
    fontWeight: "700",
    color: colors.textPrimary,
    marginBottom: 8,
  },
  sectionDescription: {
    fontSize: 14,
    color: colors.textSecondary,
    lineHeight: 22,
    marginBottom: 16,
  },
  accountInfo: {
    marginBottom: 16,
    gap: 12,
  },
  accountRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  accountLabel: {
    fontSize: 14,
    fontWeight: "600",
    color: colors.textSecondary,
  },
  accountValue: {
    fontSize: 14,
    color: colors.textPrimary,
    fontWeight: "500",
  },
  tierBadge: {
    fontSize: 13,
    fontWeight: "700",
    color: colors.accentTeal,
    textTransform: "uppercase",
    letterSpacing: 1,
    backgroundColor: "rgba(45, 212, 191, 0.1)",
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
    overflow: "hidden",
  },
  buttonRow: {
    flexDirection: "row",
    gap: 12,
  },
  primaryButton: {
    backgroundColor: colors.accentTeal,
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    flex: 1,
    alignItems: "center",
  },
  primaryButtonText: {
    color: colors.bgPrimary,
    fontSize: 16,
    fontWeight: "600",
  },
  secondaryButton: {
    backgroundColor: colors.bgTertiary,
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
    flex: 1,
    alignItems: "center",
  },
  secondaryButtonText: {
    color: colors.textPrimary,
    fontSize: 16,
    fontWeight: "600",
  },
  dangerButton: {
    backgroundColor: "transparent",
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.accentRed,
    alignItems: "center",
  },
  dangerButtonText: {
    color: colors.accentRed,
    fontSize: 16,
    fontWeight: "600",
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "center",
    marginTop: 8,
    marginBottom: 16,
    gap: 8,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: colors.statusActive,
  },
  statusText: {
    color: colors.statusActive,
    fontSize: 14,
    fontWeight: "500",
  },
  pendingText: {
    color: colors.accentAmber,
    fontSize: 13,
    marginBottom: 16,
    marginLeft: 16,
  },
  footer: {
    marginTop: "auto",
    paddingTop: 32,
    alignItems: "center",
  },
  footerText: {
    color: colors.textMuted,
    fontSize: 13,
  },
  // Scanner modal
  scannerContainer: {
    flex: 1,
    backgroundColor: colors.bgPrimary,
  },
  scannerHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    paddingHorizontal: 20,
    paddingVertical: 16,
  },
  scannerTitle: {
    color: colors.textPrimary,
    fontSize: 18,
    fontWeight: "700",
  },
  scannerClose: {
    color: colors.accentTeal,
    fontSize: 16,
    fontWeight: "600",
  },
  scannerHint: {
    color: colors.textSecondary,
    fontSize: 14,
    textAlign: "center",
    paddingHorizontal: 32,
    marginBottom: 16,
  },
  scanner: {
    flex: 1,
    marginHorizontal: 20,
    borderRadius: 16,
    overflow: "hidden",
  },
  pairingText: {
    color: colors.textSecondary,
    fontSize: 16,
    marginTop: 16,
  },
});
