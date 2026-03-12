import { useState, useEffect, useRef } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  Alert,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  Modal,
  ActivityIndicator,
} from "react-native";
import * as SecureStore from "expo-secure-store";
import { CameraView } from "expo-camera";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../../constants/theme";
import {
  parseQrPayload,
  pairWithDesktop,
  getDesktopStatus,
  loadPairing,
  clearPairing,
  type PairingInfo,
  type DesktopStatus,
} from "../../lib/desktop-client";

const API_KEY_STORAGE_KEY = "noah_api_key";

export default function SettingsTab() {
  const [apiKey, setApiKey] = useState("");
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Pairing state
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [desktopStatus, setDesktopStatus] = useState<DesktopStatus | null>(null);
  const [showScanner, setShowScanner] = useState(false);
  const [isPairing, setIsPairing] = useState(false);
  const scannedRef = useRef(false);

  useEffect(() => {
    loadStoredKey();
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

  async function loadStoredKey() {
    try {
      const stored = await SecureStore.getItemAsync(API_KEY_STORAGE_KEY);
      if (stored) {
        setHasStoredKey(true);
        setApiKey(maskKey(stored));
      }
    } catch {
      // SecureStore not available
    } finally {
      setIsLoading(false);
    }
  }

  async function loadPairingState() {
    const p = await loadPairing();
    setPairing(p);
  }

  function maskKey(key: string): string {
    if (key.length <= 8) return "****";
    return key.slice(0, 4) + "****" + key.slice(-4);
  }

  async function handleSave() {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      Alert.alert("Empty Key", "Please enter an API key before saving.");
      return;
    }
    if (hasStoredKey && trimmed === maskKey(trimmed)) {
      Alert.alert("No Changes", "The API key has not been modified.");
      return;
    }
    try {
      await SecureStore.setItemAsync(API_KEY_STORAGE_KEY, trimmed);
      setHasStoredKey(true);
      setApiKey(maskKey(trimmed));
      Alert.alert("Saved", "API key stored securely.");
    } catch {
      Alert.alert("Error", "Failed to save API key.");
    }
  }

  async function handleClear() {
    Alert.alert("Clear API Key", "Remove the stored API key?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Clear",
        style: "destructive",
        onPress: async () => {
          try {
            await SecureStore.deleteItemAsync(API_KEY_STORAGE_KEY);
            setApiKey("");
            setHasStoredKey(false);
          } catch {
            Alert.alert("Error", "Failed to clear API key.");
          }
        },
      },
    ]);
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

  if (isLoading) {
    return (
      <SafeAreaView style={styles.container} edges={["bottom"]}>
        <View style={styles.centered}>
          <Text style={styles.loadingText}>Loading...</Text>
        </View>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container} edges={["bottom"]}>
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        style={styles.flex}
      >
        <ScrollView
          contentContainerStyle={styles.scrollContent}
          keyboardShouldPersistTaps="handled"
        >
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
                <TouchableOpacity
                  style={styles.primaryButton}
                  onPress={() => {
                    scannedRef.current = false;
                    setShowScanner(true);
                  }}
                >
                  <Text style={styles.primaryButtonText}>Scan QR Code</Text>
                </TouchableOpacity>
              </>
            )}
          </View>

          {/* API Configuration Section */}
          <View style={styles.section}>
            <Text style={styles.sectionTitle}>API Configuration</Text>
            <Text style={styles.sectionDescription}>
              Enter your API key to enable photo analysis. The key is stored
              securely on your device.
            </Text>

            <View style={styles.inputGroup}>
              <Text style={styles.label}>API Key</Text>
              <TextInput
                style={styles.input}
                value={apiKey}
                onChangeText={setApiKey}
                onFocus={() => {
                  if (hasStoredKey) setApiKey("");
                }}
                placeholder="sk-..."
                placeholderTextColor={colors.textMuted}
                secureTextEntry={!hasStoredKey}
                autoCapitalize="none"
                autoCorrect={false}
                autoComplete="off"
              />
            </View>

            <View style={styles.buttonRow}>
              <TouchableOpacity style={styles.primaryButton} onPress={handleSave}>
                <Text style={styles.primaryButtonText}>Save</Text>
              </TouchableOpacity>
              {hasStoredKey && (
                <TouchableOpacity style={styles.dangerButton} onPress={handleClear}>
                  <Text style={styles.dangerButtonText}>Clear</Text>
                </TouchableOpacity>
              )}
            </View>

            {hasStoredKey && (
              <View style={styles.statusRow}>
                <View style={styles.statusDot} />
                <Text style={styles.statusText}>API key is stored</Text>
              </View>
            )}
          </View>

          <View style={styles.footer}>
            <Text style={styles.footerText}>Noah v1.0.0</Text>
          </View>
        </ScrollView>
      </KeyboardAvoidingView>

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
  flex: {
    flex: 1,
  },
  centered: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
  },
  loadingText: {
    color: colors.textSecondary,
    fontSize: 16,
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
  inputGroup: {
    marginBottom: 20,
  },
  label: {
    fontSize: 14,
    fontWeight: "600",
    color: colors.textSecondary,
    marginBottom: 8,
  },
  input: {
    backgroundColor: colors.bgInput,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
    paddingHorizontal: 16,
    paddingVertical: 14,
    fontSize: 16,
    color: colors.textPrimary,
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
  dangerButton: {
    backgroundColor: "transparent",
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.accentRed,
    flex: 1,
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
    marginTop: 16,
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
    marginTop: 8,
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
