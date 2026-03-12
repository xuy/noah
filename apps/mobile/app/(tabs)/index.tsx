import { useState, useRef } from "react";
import {
  View,
  Text,
  TouchableOpacity,
  StyleSheet,
  Image,
  Alert,
} from "react-native";
import { CameraView, useCameraPermissions } from "expo-camera";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../../constants/theme";

export default function CameraTab() {
  const [permission, requestPermission] = useCameraPermissions();
  const [photo, setPhoto] = useState<string | null>(null);
  const [analysisResult, setAnalysisResult] = useState<string | null>(null);
  const cameraRef = useRef<CameraView>(null);

  // Still loading permission status
  if (!permission) {
    return (
      <SafeAreaView style={styles.container}>
        <View style={styles.centered}>
          <Text style={styles.messageText}>Initializing camera...</Text>
        </View>
      </SafeAreaView>
    );
  }

  // Permission not granted
  if (!permission.granted) {
    return (
      <SafeAreaView style={styles.container}>
        <View style={styles.centered}>
          <Text style={styles.titleText}>Camera Access Required</Text>
          <Text style={styles.messageText}>
            Noah needs camera access to capture photos for analysis.
          </Text>
          <TouchableOpacity style={styles.primaryButton} onPress={requestPermission}>
            <Text style={styles.primaryButtonText}>Grant Permission</Text>
          </TouchableOpacity>
        </View>
      </SafeAreaView>
    );
  }

  // Captured photo review
  if (photo) {
    return (
      <SafeAreaView style={styles.container}>
        <View style={styles.previewContainer}>
          <Image source={{ uri: photo }} style={styles.previewImage} />

          {analysisResult && (
            <View style={styles.analysisCard}>
              <Text style={styles.analysisText}>{analysisResult}</Text>
            </View>
          )}

          <View style={styles.previewButtons}>
            <TouchableOpacity
              style={styles.secondaryButton}
              onPress={() => {
                setPhoto(null);
                setAnalysisResult(null);
              }}
            >
              <Text style={styles.secondaryButtonText}>Retake</Text>
            </TouchableOpacity>

            <TouchableOpacity
              style={styles.primaryButton}
              onPress={() => {
                setAnalysisResult("Analysis coming soon. This feature will be available in a future update.");
              }}
            >
              <Text style={styles.primaryButtonText}>Analyze</Text>
            </TouchableOpacity>
          </View>
        </View>
      </SafeAreaView>
    );
  }

  // Camera preview
  const handleCapture = async () => {
    if (!cameraRef.current) return;
    try {
      const result = await cameraRef.current.takePictureAsync({
        quality: 0.8,
      });
      if (result) {
        setPhoto(result.uri);
      }
    } catch (error) {
      Alert.alert("Capture Failed", "Could not take photo. Please try again.");
    }
  };

  return (
    <View style={styles.container}>
      <CameraView ref={cameraRef} style={styles.camera} facing="back">
        <SafeAreaView style={styles.cameraOverlay}>
          <View style={styles.cameraHeader}>
            <Text style={styles.cameraHeaderText}>Noah</Text>
          </View>
          <View style={styles.captureRow}>
            <TouchableOpacity
              style={styles.captureButton}
              onPress={handleCapture}
              activeOpacity={0.7}
            >
              <View style={styles.captureButtonInner} />
            </TouchableOpacity>
          </View>
        </SafeAreaView>
      </CameraView>
    </View>
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
    paddingHorizontal: 32,
  },
  titleText: {
    fontSize: 22,
    fontWeight: "700",
    color: colors.textPrimary,
    marginBottom: 12,
    textAlign: "center",
  },
  messageText: {
    fontSize: 16,
    color: colors.textSecondary,
    textAlign: "center",
    lineHeight: 24,
    marginBottom: 24,
  },
  primaryButton: {
    backgroundColor: colors.accentTeal,
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    minWidth: 140,
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
    minWidth: 140,
    alignItems: "center",
    borderWidth: 1,
    borderColor: colors.borderPrimary,
  },
  secondaryButtonText: {
    color: colors.textPrimary,
    fontSize: 16,
    fontWeight: "600",
  },
  camera: {
    flex: 1,
  },
  cameraOverlay: {
    flex: 1,
    justifyContent: "space-between",
  },
  cameraHeader: {
    alignItems: "center",
    paddingTop: 16,
  },
  cameraHeaderText: {
    color: colors.textInverse,
    fontSize: 18,
    fontWeight: "700",
    textShadowColor: "rgba(0,0,0,0.5)",
    textShadowOffset: { width: 0, height: 1 },
    textShadowRadius: 4,
  },
  captureRow: {
    alignItems: "center",
    paddingBottom: 32,
  },
  captureButton: {
    width: 76,
    height: 76,
    borderRadius: 38,
    borderWidth: 4,
    borderColor: colors.textInverse,
    justifyContent: "center",
    alignItems: "center",
  },
  captureButtonInner: {
    width: 60,
    height: 60,
    borderRadius: 30,
    backgroundColor: colors.textInverse,
  },
  previewContainer: {
    flex: 1,
  },
  previewImage: {
    flex: 1,
    resizeMode: "contain",
    backgroundColor: "#000",
  },
  previewButtons: {
    flexDirection: "row",
    justifyContent: "center",
    gap: 16,
    paddingVertical: 20,
    paddingHorizontal: 16,
    backgroundColor: colors.bgSecondary,
  },
  analysisCard: {
    backgroundColor: colors.bgSecondary,
    marginHorizontal: 16,
    marginTop: -40,
    padding: 16,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
  },
  analysisText: {
    color: colors.textSecondary,
    fontSize: 15,
    lineHeight: 22,
    textAlign: "center",
  },
});
