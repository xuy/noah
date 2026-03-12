import { useState, useCallback } from "react";
import {
  View,
  Text,
  StyleSheet,
  FlatList,
  TouchableOpacity,
  Image,
  Alert,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useFocusEffect } from "expo-router";
import { colors } from "../../constants/theme";
import { listAnalyses, deleteAnalysis, type AnalysisRecord } from "../../lib/history-db";

export default function HistoryTab() {
  const [analyses, setAnalyses] = useState<AnalysisRecord[]>([]);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  useFocusEffect(
    useCallback(() => {
      listAnalyses().then(setAnalyses);
    }, []),
  );

  function handleDelete(id: number) {
    Alert.alert("Delete Analysis", "Remove this analysis from history?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Delete",
        style: "destructive",
        onPress: async () => {
          await deleteAnalysis(id);
          setAnalyses((prev) => prev.filter((a) => a.id !== id));
          if (expandedId === id) setExpandedId(null);
        },
      },
    ]);
  }

  if (analyses.length === 0) {
    return (
      <SafeAreaView style={styles.container} edges={["bottom"]}>
        <View style={styles.centered}>
          <Text style={styles.emptyTitle}>No analyses yet</Text>
          <Text style={styles.emptyMessage}>
            Take a photo to get started.
          </Text>
        </View>
      </SafeAreaView>
    );
  }

  function formatDate(iso: string): string {
    const d = new Date(iso + "Z");
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return (
    <SafeAreaView style={styles.container} edges={["bottom"]}>
      <FlatList
        data={analyses}
        keyExtractor={(item) => String(item.id)}
        contentContainerStyle={styles.listContent}
        renderItem={({ item }) => {
          const expanded = expandedId === item.id;
          const preview = item.analysis.slice(0, 120) + (item.analysis.length > 120 ? "..." : "");
          return (
            <TouchableOpacity
              style={styles.card}
              onPress={() => setExpandedId(expanded ? null : item.id)}
              onLongPress={() => handleDelete(item.id)}
              activeOpacity={0.8}
            >
              <View style={styles.cardHeader}>
                <Image source={{ uri: item.photo_uri }} style={styles.thumbnail} />
                <View style={styles.cardMeta}>
                  <Text style={styles.cardDate}>{formatDate(item.created_at)}</Text>
                  <Text style={styles.cardModel}>{item.model}</Text>
                </View>
              </View>
              <Text style={styles.cardText}>
                {expanded ? item.analysis : preview}
              </Text>
              {expanded && (
                <TouchableOpacity
                  style={styles.deleteButton}
                  onPress={() => handleDelete(item.id)}
                >
                  <Text style={styles.deleteButtonText}>Delete</Text>
                </TouchableOpacity>
              )}
            </TouchableOpacity>
          );
        }}
      />
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
    paddingHorizontal: 32,
  },
  emptyTitle: {
    fontSize: 20,
    fontWeight: "700",
    color: colors.textPrimary,
    marginBottom: 8,
    textAlign: "center",
  },
  emptyMessage: {
    fontSize: 16,
    color: colors.textSecondary,
    textAlign: "center",
    lineHeight: 24,
  },
  listContent: {
    padding: 16,
    gap: 12,
  },
  card: {
    backgroundColor: colors.bgSecondary,
    borderRadius: 12,
    padding: 16,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
  },
  cardHeader: {
    flexDirection: "row",
    marginBottom: 12,
    gap: 12,
  },
  thumbnail: {
    width: 56,
    height: 56,
    borderRadius: 8,
    backgroundColor: "#000",
  },
  cardMeta: {
    flex: 1,
    justifyContent: "center",
  },
  cardDate: {
    color: colors.textPrimary,
    fontSize: 15,
    fontWeight: "600",
  },
  cardModel: {
    color: colors.textMuted,
    fontSize: 12,
    marginTop: 2,
  },
  cardText: {
    color: colors.textSecondary,
    fontSize: 14,
    lineHeight: 20,
  },
  deleteButton: {
    marginTop: 12,
    alignSelf: "flex-end",
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: colors.accentRed,
  },
  deleteButtonText: {
    color: colors.accentRed,
    fontSize: 13,
    fontWeight: "600",
  },
});
