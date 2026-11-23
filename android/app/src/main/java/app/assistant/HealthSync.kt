package app.assistant

import android.content.Context
import android.os.Build
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.aggregate.AggregateMetric
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ActiveCaloriesBurnedRecord
import androidx.health.connect.client.records.DistanceRecord
import androidx.health.connect.client.records.ExerciseSessionRecord
import androidx.health.connect.client.records.FloorsClimbedRecord
import androidx.health.connect.client.records.HeartRateRecord
import androidx.health.connect.client.records.HydrationRecord
import androidx.health.connect.client.records.RestingHeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.TotalCaloriesBurnedRecord
import androidx.health.connect.client.records.WeightRecord
import androidx.health.connect.client.request.AggregateRequest
import androidx.health.connect.client.time.TimeRangeFilter
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.util.concurrent.Executors
import kotlin.reflect.KClass

/**
 * Body numbers from Android's Health Connect (docs/11). After you allow it there, the app adds up
 * each day (steps, sleep, heart rate, weight, workouts) and sends one small row per day to your own
 * server. Raw readings never leave the phone. It sends when the app opens, at most every 10 minutes.
 */
object HealthSync {
    const val KEY_ON = "health_on"
    private const val KEY_LAST = "health_last"
    private const val KEY_DONE_ONCE = "health_first_done"
    private const val GAP_MS = 10 * 60_000L
    private val io = Executors.newSingleThreadExecutor()

    @Volatile var listener: (() -> Unit)? = null
    @Volatile var running = false

    /** Every kind of record we read, with its permission name. */
    private val READS: List<KClass<out androidx.health.connect.client.records.Record>> = listOf(
        StepsRecord::class, DistanceRecord::class, FloorsClimbedRecord::class, ActiveCaloriesBurnedRecord::class,
        TotalCaloriesBurnedRecord::class, SleepSessionRecord::class, HeartRateRecord::class, RestingHeartRateRecord::class,
        WeightRecord::class, ExerciseSessionRecord::class, HydrationRecord::class,
    )

    val permissions: Set<String> get() = READS.map { HealthPermission.getReadPermission(it) }.toSet()

    fun prefs(ctx: Context) = ctx.getSharedPreferences("app", Context.MODE_PRIVATE)
    fun on(ctx: Context) = prefs(ctx).getBoolean(KEY_ON, false)
    fun available(ctx: Context) = HealthConnectClient.getSdkStatus(ctx) == HealthConnectClient.SDK_AVAILABLE

    /** "5 min ago" style text for the screen, or null if never. */
    fun last(ctx: Context): String? {
        val t = prefs(ctx).getLong(KEY_LAST, 0)
        if (t == 0L) return null
        val m = (System.currentTimeMillis() - t) / 60_000
        return if (m < 1) "just now" else if (m < 120) "$m min ago" else "${m / 60} h ago"
    }

    /** The permissions the user has not allowed yet. Blocking: call off the main thread. */
    fun missing(ctx: Context): Set<String> = try {
        runBlocking { permissions - HealthConnectClient.getOrCreate(ctx).permissionController.getGrantedPermissions() }
    } catch (e: Exception) { permissions }

    /** Turn off and forget when it last sent. Nothing already on the server is removed here. */
    fun off(ctx: Context) {
        prefs(ctx).edit().putBoolean(KEY_ON, false).apply()
        listener?.invoke()
    }

    /** Send in the background if it is on and was not sent a moment ago. */
    fun syncSoon(ctx: Context, force: Boolean = false) {
        if (!on(ctx) || Build.VERSION.SDK_INT < 31 || running) return
        if (!force && System.currentTimeMillis() - prefs(ctx).getLong(KEY_LAST, 0) < GAP_MS) return
        running = true
        io.execute {
            try { sync(ctx) } catch (e: Exception) { } finally { running = false; listener?.invoke() }
        }
    }

    private fun sync(ctx: Context) {
        if (!available(ctx) || Alarms.token(ctx).isEmpty()) return
        val client = HealthConnectClient.getOrCreate(ctx)
        val granted = runBlocking { client.permissionController.getGrantedPermissions() }
        if (granted.none { it in permissions }) return
        // A sleep session that ended this morning means the user woke up (the morning brief, Woke).
        if (HealthPermission.getReadPermission(SleepSessionRecord::class) in granted) try {
            val now = java.time.Instant.now()
            val r = runBlocking { client.readRecords(androidx.health.connect.client.request.ReadRecordsRequest(SleepSessionRecord::class, TimeRangeFilter.between(now.minusSeconds(14 * 3600), now))) }
            r.records.maxOfOrNull { it.endTime }?.let { Woke.sleepEnded(ctx, it.toEpochMilli()) }
        } catch (e: Exception) { }
        val first = !prefs(ctx).getBoolean(KEY_DONE_ONCE, false)
        val back = if (first) 30 else 3 // days; today is always included
        val today = LocalDate.now()
        val days = JSONArray()
        for (i in 0..back) {
            val d = today.minusDays(i.toLong())
            val m = runBlocking { daily(client, granted, d) }
            if (m.length() > 0) days.put(JSONObject().put("day", d.toString()).put("metrics", m))
        }
        if (days.length() == 0) return
        for (part in (0 until days.length()).map { days.getJSONObject(it) }.chunked(60)) {
            val body = JSONObject().put("days", JSONArray(part)).toString()
            val code = Alarms.http(ctx, "POST", "/v1/health/days", body).first
            if (code != 200) return // try again the next time the app opens
        }
        prefs(ctx).edit().putLong(KEY_LAST, System.currentTimeMillis()).putBoolean(KEY_DONE_ONCE, true).apply()
    }

    /** One day's totals, asking only for the kinds the user allowed. */
    private suspend fun daily(client: HealthConnectClient, granted: Set<String>, day: LocalDate): JSONObject {
        val out = JSONObject()
        fun can(k: KClass<out androidx.health.connect.client.records.Record>) = HealthPermission.getReadPermission(k) in granted
        val start = LocalDateTime.of(day, LocalTime.MIN)
        val end = start.plusDays(1)
        val wanted = mutableSetOf<AggregateMetric<*>>()
        if (can(StepsRecord::class)) wanted += StepsRecord.COUNT_TOTAL
        if (can(DistanceRecord::class)) wanted += DistanceRecord.DISTANCE_TOTAL
        if (can(FloorsClimbedRecord::class)) wanted += FloorsClimbedRecord.FLOORS_CLIMBED_TOTAL
        if (can(ActiveCaloriesBurnedRecord::class)) wanted += ActiveCaloriesBurnedRecord.ACTIVE_CALORIES_TOTAL
        if (can(TotalCaloriesBurnedRecord::class)) wanted += TotalCaloriesBurnedRecord.ENERGY_TOTAL
        if (can(HeartRateRecord::class)) { wanted += HeartRateRecord.BPM_AVG; wanted += HeartRateRecord.BPM_MAX }
        if (can(RestingHeartRateRecord::class)) wanted += RestingHeartRateRecord.BPM_AVG
        if (can(WeightRecord::class)) wanted += WeightRecord.WEIGHT_AVG
        if (can(ExerciseSessionRecord::class)) wanted += ExerciseSessionRecord.EXERCISE_DURATION_TOTAL
        if (can(HydrationRecord::class)) wanted += HydrationRecord.VOLUME_TOTAL
        if (wanted.isNotEmpty()) {
            val r = client.aggregate(AggregateRequest(wanted, TimeRangeFilter.between(start, end)))
            r[StepsRecord.COUNT_TOTAL]?.let { out.put("steps", it) }
            r[DistanceRecord.DISTANCE_TOTAL]?.let { out.put("distance_m", it.inMeters) }
            r[FloorsClimbedRecord.FLOORS_CLIMBED_TOTAL]?.let { out.put("floors", it) }
            r[ActiveCaloriesBurnedRecord.ACTIVE_CALORIES_TOTAL]?.let { out.put("active_kcal", it.inKilocalories) }
            r[TotalCaloriesBurnedRecord.ENERGY_TOTAL]?.let { out.put("total_kcal", it.inKilocalories) }
            r[HeartRateRecord.BPM_AVG]?.let { out.put("avg_hr", it) }
            r[HeartRateRecord.BPM_MAX]?.let { out.put("max_hr", it) }
            r[RestingHeartRateRecord.BPM_AVG]?.let { out.put("resting_hr", it) }
            r[WeightRecord.WEIGHT_AVG]?.let { out.put("weight_kg", it.inKilograms) }
            r[ExerciseSessionRecord.EXERCISE_DURATION_TOTAL]?.let { out.put("exercise_min", it.toMinutes()) }
            r[HydrationRecord.VOLUME_TOTAL]?.let { out.put("hydration_ml", it.inMilliliters) }
        }
        // A night counts for the day you wake up: from 6 pm the evening before to 6 pm that day.
        if (can(SleepSessionRecord::class)) {
            val s = day.minusDays(1).atTime(18, 0)
            val r = client.aggregate(AggregateRequest(setOf(SleepSessionRecord.SLEEP_DURATION_TOTAL), TimeRangeFilter.between(s, s.plusDays(1))))
            r[SleepSessionRecord.SLEEP_DURATION_TOTAL]?.let { out.put("sleep_min", it.toMinutes()) }
        }
        // Zero means "nothing recorded" for most of these; leave them out so the server keeps what it had.
        val keys = out.keys().asSequence().toList()
        for (k in keys) if (out.optDouble(k, 0.0) == 0.0) out.remove(k)
        return out
    }
}
