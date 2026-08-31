package org.zioncode.app

import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

private enum class Screen {
    Import,
    Reader,
    About,
}

private const val BLIVRE_SITE = "http://sites.google.com/site/biblialivre/"
private const val BLIVRE_LICENSE = "https://creativecommons.org/licenses/by/3.0/br/"
internal const val READ_EXTERNAL_STORAGE_PERMISSION = "android.permission.READ_EXTERNAL_STORAGE"
internal const val READ_MEDIA_IMAGES_PERMISSION = "android.permission.READ_MEDIA_IMAGES"
internal const val READ_MEDIA_VISUAL_USER_SELECTED_PERMISSION =
    "android.permission.READ_MEDIA_VISUAL_USER_SELECTED"
private const val BLIVRE_ATTRIBUTION =
    "Todas as Escrituras em português citadas são da Bíblia Livre (BLIVRE), Copyright © Diego Santos, Mario Sérgio, e Marco Teles, http://sites.google.com/site/biblialivre/ - fevereiro de 2018. Licença Creative Commons Atribuição 3.0 Brasil (https://creativecommons.org/licenses/by/3.0/br/). The normalized JSON retains byte-exact, exportable source USFM and its pinned provenance."

@Composable
internal fun ZionArcApp() {
    val context = LocalContext.current
    val repository = remember(context) { ArcRepository(context.contentResolver) }
    val scope = rememberCoroutineScope()
    var screen by remember { mutableStateOf(Screen.Import) }
    var selectedUri by remember { mutableStateOf<Uri?>(null) }
    var password by remember { mutableStateOf("") }
    var loading by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    var book by remember { mutableStateOf<BookUiModel?>(null) }
    var selectedDocumentInfo by remember { mutableStateOf<SelectedDocumentInfo?>(null) }
    var imageAccessGranted by rememberSaveable { mutableStateOf<Boolean?>(null) }
    var initialAccessFlowRequested by rememberSaveable { mutableStateOf(false) }
    var openPickerAfterPermission by remember { mutableStateOf(false) }

    val openDocument = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        selectedUri = uri
        selectedDocumentInfo = null
        errorMessage = null
        notice = if (uri == null) null else "Arte PNG conectada. Agora insira a chave de acesso."
        if (uri != null) {
            scope.launch {
                val info = repository.describe(uri)
                if (selectedUri == uri) selectedDocumentInfo = info
            }
        }
    }
    val createDocument = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("text/plain"),
    ) { uri ->
        val currentBook = book
        if (uri != null && currentBook != null) {
            scope.launch {
                notice = try {
                    repository.exportRawUsfm(uri, currentBook.rawUsfm)
                    "USFM original exportado sem alteração."
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (_: Exception) {
                    "Não foi possível exportar o USFM."
                }
            }
        }
    }
    val requestImageAccess = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        val granted = grants.values.any { it }
        imageAccessGranted = granted
        notice = if (granted) {
            "Permissão de imagens concedida. Escolha agora a arte PNG."
        } else {
            "Permissão de imagens negada. Você ainda pode liberar somente um PNG pelo painel do Android."
        }
        openPickerAfterPermission = true
    }

    LaunchedEffect(Unit) {
        if (shouldStartInitialAccessFlow(initialAccessFlowRequested, selectedUri != null)) {
            initialAccessFlowRequested = true
            requestImageAccess.launch(requiredImagePermissions(Build.VERSION.SDK_INT))
        }
    }
    LaunchedEffect(openPickerAfterPermission) {
        if (openPickerAfterPermission) {
            openPickerAfterPermission = false
            openDocument.launch(arrayOf("image/png"))
        }
    }

    ZionArcTheme {
        ZionBackdrop {
            Scaffold(
                containerColor = Color.Transparent,
                topBar = {
                    AppBar(
                        screen = screen,
                        onHome = { screen = if (book == null) Screen.Import else Screen.Reader },
                        onAbout = { screen = Screen.About },
                    )
                },
            ) { padding ->
                when (screen) {
                    Screen.Import -> ImportScreen(
                        accessGranted = imageAccessGranted,
                        selected = selectedUri != null,
                        documentInfo = selectedDocumentInfo,
                        password = password,
                        loading = loading,
                        errorMessage = errorMessage,
                        notice = notice,
                        onPasswordChanged = { candidate ->
                            if (isPassphraseInputWithinLimit(candidate)) {
                                password = candidate
                            }
                        },
                        onSelect = { openDocument.launch(arrayOf("image/png")) },
                        onOpen = {
                            val uri = selectedUri ?: return@ImportScreen
                            val passphraseBytes = password.toByteArray(Charsets.UTF_8)
                            if (passphraseBytes.size !in 1..MAX_ARC_PASSPHRASE_BYTES) {
                                passphraseBytes.fill(0)
                                errorMessage = "A senha deve ter entre 1 e 1.024 bytes UTF-8."
                                return@ImportScreen
                            }
                            password = ""
                            loading = true
                            errorMessage = null
                            notice = null
                            scope.launch {
                                try {
                                    book = repository.decode(uri, passphraseBytes)
                                    screen = Screen.Reader
                                } catch (cancelled: CancellationException) {
                                    throw cancelled
                                } catch (failure: Exception) {
                                    errorMessage = userFacingDecodeError(failure)
                                } finally {
                                    passphraseBytes.fill(0)
                                    loading = false
                                }
                            }
                        },
                        modifier = Modifier.padding(padding),
                    )
                    Screen.Reader -> ReaderScreen(
                        book = book,
                        notice = notice,
                        onExport = {
                            val currentBook = book ?: return@ReaderScreen
                            createDocument.launch("${currentBook.id}.usfm")
                        },
                        onLock = {
                            book = null
                            selectedUri = null
                            selectedDocumentInfo = null
                            password = ""
                            errorMessage = null
                            notice = null
                            screen = Screen.Import
                        },
                        modifier = Modifier.padding(padding),
                    )
                    Screen.About -> AboutScreen(modifier = Modifier.padding(padding))
                }
            }
        }
    }
}

internal fun shouldStartInitialAccessFlow(alreadyRequested: Boolean, hasSelection: Boolean): Boolean =
    !alreadyRequested && !hasSelection

internal fun requiredImagePermissions(sdkInt: Int): Array<String> = when {
    sdkInt >= 34 -> arrayOf(
        READ_MEDIA_IMAGES_PERMISSION,
        READ_MEDIA_VISUAL_USER_SELECTED_PERMISSION,
    )
    sdkInt >= 33 -> arrayOf(READ_MEDIA_IMAGES_PERMISSION)
    else -> arrayOf(READ_EXTERNAL_STORAGE_PERMISSION)
}

@Composable
private fun ZionBackdrop(content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    listOf(Color(0xFF03131C), Color(0xFF071D29), Color(0xFF030C12)),
                ),
            ),
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(ZionCyan.copy(alpha = 0.17f), Color.Transparent),
                    center = Offset(size.width * 0.82f, size.height * 0.08f),
                    radius = size.width * 0.72f,
                ),
                radius = size.width * 0.72f,
                center = Offset(size.width * 0.82f, size.height * 0.08f),
            )
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(ZionGold.copy(alpha = 0.08f), Color.Transparent),
                    center = Offset(size.width * 0.08f, size.height * 0.76f),
                    radius = size.width * 0.55f,
                ),
                radius = size.width * 0.55f,
                center = Offset(size.width * 0.08f, size.height * 0.76f),
            )
            repeat(18) { index ->
                val x = size.width * (((index * 37) % 97) / 97f)
                val y = size.height * (((index * 53 + 11) % 101) / 101f)
                drawCircle(
                    color = ZionCyanSoft.copy(alpha = if (index % 3 == 0) 0.24f else 0.11f),
                    radius = if (index % 4 == 0) 2.2f else 1.2f,
                    center = Offset(x, y),
                )
            }
        }
        content()
    }
}

@Composable
private fun AppBar(screen: Screen, onHome: () -> Unit, onAbout: () -> Unit) {
    Surface(color = ZionNight.copy(alpha = 0.86f)) {
        Column {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 18.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ArcMark(modifier = Modifier.size(34.dp))
                Spacer(modifier = Modifier.width(10.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text("ZION ARC", style = MaterialTheme.typography.titleMedium)
                    Text(
                        text = "LEITOR ÓPTICO OFFLINE",
                        color = ZionMuted,
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
                if (screen == Screen.About) {
                    TextButton(onClick = onHome) { Text("Voltar") }
                } else {
                    TextButton(onClick = onAbout) { Text("Sobre") }
                }
            }
            HorizontalDivider(color = ZionLine.copy(alpha = 0.55f))
        }
    }
}

@Composable
private fun ArcMark(modifier: Modifier = Modifier) {
    Canvas(modifier = modifier) {
        val stroke = size.minDimension * 0.075f
        drawCircle(
            brush = Brush.radialGradient(listOf(ZionCyan.copy(alpha = 0.28f), Color.Transparent)),
        )
        drawArc(
            color = ZionCyan,
            startAngle = 202f,
            sweepAngle = 265f,
            useCenter = false,
            style = Stroke(width = stroke, cap = StrokeCap.Round),
        )
        drawArc(
            color = ZionGold,
            startAngle = 28f,
            sweepAngle = 92f,
            useCenter = false,
            style = Stroke(width = stroke, cap = StrokeCap.Round),
        )
        drawCircle(color = ZionInk, radius = size.minDimension * 0.075f)
    }
}

@Composable
private fun OpticalHero(accessGranted: Boolean?, selected: Boolean, loading: Boolean) {
    val transition = rememberInfiniteTransition(label = "optical hero")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 3_200, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "optical scan",
    )
    val shape = RoundedCornerShape(28.dp)
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(190.dp)
            .clip(shape)
            .background(
                Brush.linearGradient(
                    listOf(Color(0xFF0B3442), Color(0xFF071923), Color(0xFF102D35)),
                ),
            )
            .border(1.dp, ZionCyan.copy(alpha = 0.38f), shape),
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            val orbitCenter = Offset(size.width * 0.74f, size.height * 0.51f)
            val scanY = size.height * (0.12f + phase * 0.76f)
            drawLine(
                color = ZionCyan.copy(alpha = 0.2f),
                start = Offset(0f, scanY),
                end = Offset(size.width, scanY),
                strokeWidth = 2f,
            )
            repeat(10) { column ->
                repeat(6) { row ->
                    val active = (column * 7 + row * 11) % 9 == (phase * 9).toInt()
                    drawCircle(
                        color = if (active) ZionGold else ZionCyan,
                        radius = if (active) 2.6f else 1.4f,
                        center = Offset(
                            size.width * (0.05f + column * 0.095f),
                            size.height * (0.12f + row * 0.15f),
                        ),
                        alpha = if (active) 0.7f else 0.11f,
                    )
                }
            }
            repeat(3) { ring ->
                drawCircle(
                    color = ZionCyan.copy(alpha = 0.12f - ring * 0.025f),
                    radius = size.minDimension * (0.2f + ring * 0.1f),
                    center = orbitCenter,
                    style = Stroke(width = 1.5f),
                )
            }
            rotate(degrees = phase * 360f, pivot = orbitCenter) {
                val radius = size.minDimension * 0.34f
                drawArc(
                    color = ZionCyan.copy(alpha = 0.72f),
                    startAngle = 12f,
                    sweepAngle = 112f,
                    useCenter = false,
                    topLeft = Offset(orbitCenter.x - radius, orbitCenter.y - radius),
                    size = Size(radius * 2f, radius * 2f),
                    style = Stroke(width = 3f, cap = StrokeCap.Round),
                )
                drawArc(
                    color = ZionGold.copy(alpha = 0.85f),
                    startAngle = 192f,
                    sweepAngle = 48f,
                    useCenter = false,
                    topLeft = Offset(orbitCenter.x - radius, orbitCenter.y - radius),
                    size = Size(radius * 2f, radius * 2f),
                    style = Stroke(width = 3f, cap = StrokeCap.Round),
                )
            }
        }
        Column(
            modifier = Modifier
                .align(Alignment.CenterStart)
                .width(180.dp)
                .padding(start = 18.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                "ZARC / SIGNAL 01",
                color = ZionCyanSoft,
                style = MaterialTheme.typography.labelSmall,
            )
            Text(
                "OPTICAL\nCAPSULE",
                color = ZionInk,
                style = MaterialTheme.typography.headlineSmall,
            )
            OpticalStatus(
                text = when {
                    loading -> "RECUPERANDO"
                    selected -> "ARTE CONECTADA"
                    accessGranted == true -> "AGUARDANDO PNG"
                    accessGranted == false -> "ACESSO LIMITADO"
                    else -> "INICIANDO ACESSO"
                },
                active = selected || loading,
            )
        }
        ArcMark(
            modifier = Modifier
                .align(Alignment.CenterEnd)
                .padding(end = 28.dp)
                .size(82.dp),
        )
    }
}

@Composable
private fun OpticalStatus(text: String, active: Boolean) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(8.dp)
                .background(if (active) ZionSuccess else ZionGold, CircleShape),
        )
        Text(
            text,
            color = if (active) ZionSuccess else ZionGold,
            style = MaterialTheme.typography.labelSmall,
        )
    }
}

@Composable
private fun AccessJourney(accessGranted: Boolean?, selected: Boolean, unlocked: Boolean) {
    Surface(
        color = ZionNight.copy(alpha = 0.62f),
        shape = RoundedCornerShape(18.dp),
        border = BorderStroke(1.dp, ZionLine.copy(alpha = 0.72f)),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 10.dp, vertical = 14.dp),
        ) {
            JourneyStep(
                "01",
                "ACESSO",
                complete = accessGranted == true,
                active = accessGranted == null,
                failed = accessGranted == false,
                modifier = Modifier.weight(1f),
            )
            JourneyStep(
                "02",
                "ARTE",
                complete = selected,
                active = accessGranted != null && !selected,
                modifier = Modifier.weight(1f),
            )
            JourneyStep(
                "03",
                "CHAVE",
                complete = unlocked,
                active = selected && !unlocked,
                modifier = Modifier.weight(1f),
            )
        }
    }
}

@Composable
private fun JourneyStep(
    number: String,
    label: String,
    complete: Boolean,
    active: Boolean,
    modifier: Modifier = Modifier,
    failed: Boolean = false,
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Surface(
            modifier = Modifier.size(32.dp),
            shape = CircleShape,
            color = when {
                complete -> ZionSuccess.copy(alpha = 0.15f)
                failed -> ZionError.copy(alpha = 0.12f)
                active -> ZionCyan.copy(alpha = 0.13f)
                else -> Color.Transparent
            },
            border = BorderStroke(
                1.dp,
                when {
                    complete -> ZionSuccess
                    failed -> ZionError
                    active -> ZionCyan
                    else -> ZionLine
                },
            ),
        ) {
            Box(contentAlignment = Alignment.Center) {
                Text(
                    when {
                        complete -> "✓"
                        failed -> "!"
                        else -> number
                    },
                    color = when {
                        complete -> ZionSuccess
                        failed -> ZionError
                        active -> ZionCyan
                        else -> ZionMuted
                    },
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
        Text(
            label,
            color = if (active || complete || failed) ZionInk else ZionMuted,
            style = MaterialTheme.typography.labelSmall,
        )
    }
}

@Composable
private fun ImportScreen(
    accessGranted: Boolean?,
    selected: Boolean,
    documentInfo: SelectedDocumentInfo?,
    password: String,
    loading: Boolean,
    errorMessage: String?,
    notice: String?,
    onPasswordChanged: (String) -> Unit,
    onSelect: () -> Unit,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        item {
            OpticalHero(accessGranted = accessGranted, selected = selected, loading = loading)
            Spacer(modifier = Modifier.height(18.dp))
            Text(
                text = "Conhecimento que viaja como arte.",
                style = MaterialTheme.typography.displaySmall,
            )
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Um PNG comum por fora. Um livro autenticado e recuperável por dentro.",
                style = MaterialTheme.typography.bodyLarge,
                color = ZionMuted,
            )
        }
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FeatureChip("OFFLINE", Modifier.weight(1f))
                FeatureChip("RUST CORE", Modifier.weight(1f))
                FeatureChip("RGB8", Modifier.weight(1f), accent = ZionGold)
            }
        }
        item {
            AccessJourney(
                accessGranted = accessGranted,
                selected = selected,
                unlocked = password.isNotEmpty(),
            )
        }
        item {
            ImportCard(
                selected = selected,
                documentInfo = documentInfo,
                password = password,
                loading = loading,
                onPasswordChanged = onPasswordChanged,
                onSelect = onSelect,
                onOpen = onOpen,
            )
        }
        if (loading) {
            item {
                StatusCard(color = ZionCyan) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(26.dp),
                        strokeWidth = 2.dp,
                    )
                    Text("Autenticando e reconstruindo o livro…")
                }
            }
        }
        notice?.let { message ->
            item { StatusCard(color = ZionSuccess) { Text(message) } }
        }
        errorMessage?.let { message ->
            item { StatusCard(color = ZionError) { Text(message) } }
        }
        item { PrivacyCard() }
        item {
            Text(
                text = "PNG ORIGINAL  •  PIXELS PRESERVADOS  •  LEITURA OFFLINE",
                modifier = Modifier.fillMaxWidth(),
                color = ZionMuted,
                style = MaterialTheme.typography.labelSmall,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun ImportCard(
    selected: Boolean,
    documentInfo: SelectedDocumentInfo?,
    password: String,
    loading: Boolean,
    onPasswordChanged: (String) -> Unit,
    onSelect: () -> Unit,
    onOpen: () -> Unit,
) {
    Card(
        colors = CardDefaults.cardColors(containerColor = ZionPanel),
        border = BorderStroke(1.dp, ZionLine),
        shape = RoundedCornerShape(24.dp),
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                StepNumber("02", complete = selected)
                Spacer(modifier = Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text("Conecte o arquivo-arte", style = MaterialTheme.typography.titleLarge)
                    Text(
                        if (selected) "PNG pronto para autenticação" else "Escolha o arquivo original, sem screenshot",
                        color = if (selected) ZionSuccess else ZionMuted,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            if (selected) {
                SelectedDocumentCard(documentInfo)
            }
            OutlinedButton(
                onClick = onSelect,
                enabled = !loading,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(54.dp),
                shape = RoundedCornerShape(16.dp),
            ) {
                Text(if (selected) "TROCAR ARTE PNG" else "ESCOLHER ARTE PNG")
            }
            if (selected) {
                HorizontalDivider(color = ZionLine)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    StepNumber("03", complete = password.isNotEmpty())
                    Spacer(modifier = Modifier.width(12.dp))
                    Column {
                        Text("Desbloqueie no aparelho", style = MaterialTheme.typography.titleMedium)
                        Text("A chave nunca é enviada", color = ZionMuted)
                    }
                }
                OutlinedTextField(
                    value = password,
                    onValueChange = onPasswordChanged,
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Chave de acesso") },
                    placeholder = { Text("Digite a senha da cápsula") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        autoCorrectEnabled = false,
                        keyboardType = KeyboardType.Password,
                    ),
                    enabled = !loading,
                    supportingText = { Text("Até 1.024 bytes UTF-8") },
                    shape = RoundedCornerShape(16.dp),
                )
                Button(
                    onClick = onOpen,
                    enabled = password.isNotEmpty() && !loading,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(58.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = ZionCyan,
                        contentColor = Color(0xFF00221D),
                    ),
                    shape = RoundedCornerShape(16.dp),
                ) {
                    Text("ABRIR LIVRO OFFLINE")
                }
            }
        }
    }
}

@Composable
private fun SelectedDocumentCard(documentInfo: SelectedDocumentInfo?) {
    Surface(
        color = ZionCyan.copy(alpha = 0.08f),
        shape = RoundedCornerShape(16.dp),
        border = BorderStroke(1.dp, ZionCyan.copy(alpha = 0.35f)),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(48.dp)
                    .background(
                        Brush.linearGradient(listOf(ZionGold, Color(0xFFFF9F43))),
                        RoundedCornerShape(13.dp),
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    "PNG",
                    color = Color(0xFF2A1700),
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Black,
                )
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    text = documentInfo?.displayName ?: "Lendo detalhes do arquivo…",
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    text = documentInfo?.let { formatDocumentSize(it.sizeBytes) }
                        ?: "Consultando tamanho…",
                    color = ZionMuted,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            Text(
                "PRONTO",
                color = ZionSuccess,
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}

@Composable
private fun StepNumber(label: String, complete: Boolean) {
    Surface(
        modifier = Modifier.size(44.dp),
        shape = CircleShape,
        color = if (complete) ZionSuccess.copy(alpha = 0.16f) else ZionCyan.copy(alpha = 0.12f),
        border = BorderStroke(1.dp, if (complete) ZionSuccess else ZionCyan),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Text(
                text = if (complete) "✓" else label,
                color = if (complete) ZionSuccess else ZionCyan,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

@Composable
private fun ProtocolBadge(text: String) {
    Surface(
        shape = RoundedCornerShape(100.dp),
        color = ZionCyan.copy(alpha = 0.1f),
        border = BorderStroke(1.dp, ZionCyan.copy(alpha = 0.45f)),
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 7.dp),
            color = ZionCyanSoft,
            style = MaterialTheme.typography.labelSmall,
        )
    }
}

@Composable
private fun FeatureChip(text: String, modifier: Modifier = Modifier, accent: Color = ZionCyan) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(12.dp),
        color = ZionPanel,
        border = BorderStroke(1.dp, ZionLine),
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 10.dp),
            color = accent,
            style = MaterialTheme.typography.labelSmall,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun StatusCard(color: Color, content: @Composable () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(color.copy(alpha = 0.1f), RoundedCornerShape(16.dp))
            .border(1.dp, color.copy(alpha = 0.45f), RoundedCornerShape(16.dp))
            .padding(16.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        content()
    }
}

@Composable
private fun PrivacyCard() {
    Card(
        colors = CardDefaults.cardColors(containerColor = ZionNight.copy(alpha = 0.68f)),
        shape = RoundedCornerShape(18.dp),
    ) {
        Row(
            modifier = Modifier.padding(18.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("◉", color = ZionGold, style = MaterialTheme.typography.titleLarge)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Acesso controlado por você", style = MaterialTheme.typography.titleMedium)
                Text(
                    "O app solicita acesso a imagens ao abrir; depois você escolhe o PNG exato no painel do Android.",
                    color = ZionMuted,
                )
            }
        }
    }
}

@Composable
private fun ReaderScreen(
    book: BookUiModel?,
    notice: String?,
    onExport: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (book == null) {
        Column(modifier = modifier.padding(24.dp)) {
            Text("Nenhum livro aberto.")
        }
        return
    }
    val chapterCount = remember(book) {
        book.rows.count { it is ReaderRow.ChapterHeading }
    }
    val verseCount = remember(book) {
        book.rows.count { it is ReaderRow.Verse }
    }
    Column(modifier = modifier.fillMaxSize()) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            colors = CardDefaults.cardColors(containerColor = ZionPanel),
            border = BorderStroke(1.dp, ZionLine),
            shape = RoundedCornerShape(22.dp),
        ) {
            Column(
                modifier = Modifier.padding(18.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                ProtocolBadge("LIVRO AUTENTICADO")
                Row(
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    BookSeal(book.ordinal)
                    Column(
                        modifier = Modifier.weight(1f),
                        verticalArrangement = Arrangement.spacedBy(3.dp),
                    ) {
                        Text(book.title, style = MaterialTheme.typography.headlineSmall)
                        Text(
                            "${book.sourceName} • ${book.sourceVersion}",
                            style = MaterialTheme.typography.bodySmall,
                            color = ZionMuted,
                        )
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    BookMetric("$chapterCount", "CAPÍTULOS")
                    BookMetric("$verseCount", "VERSOS")
                    BookMetric(book.id, "ARQUIVO")
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = onExport, modifier = Modifier.weight(1f)) {
                        Text("Exportar USFM")
                    }
                    TextButton(onClick = onLock) { Text("Fechar livro") }
                }
                notice?.let { Text(it, color = ZionSuccess) }
            }
        }
        LazyColumn(
            contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(book.rows) { row -> ReaderRowItem(row) }
        }
    }
}

@Composable
private fun BookSeal(ordinal: Int) {
    Box(
        modifier = Modifier
            .size(62.dp)
            .background(
                Brush.linearGradient(listOf(ZionCyan, Color(0xFF1BA9B5))),
                RoundedCornerShape(18.dp),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                ordinal.toString().padStart(2, '0'),
                color = Color(0xFF002126),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Black,
            )
            Text(
                "ARC",
                color = Color(0xFF003238),
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}

@Composable
private fun BookMetric(value: String, label: String) {
    Surface(
        color = ZionNight.copy(alpha = 0.72f),
        shape = RoundedCornerShape(11.dp),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 7.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(value, color = ZionGold, style = MaterialTheme.typography.labelLarge)
            Text(label, color = ZionMuted, style = MaterialTheme.typography.labelSmall)
        }
    }
}

@Composable
private fun ReaderRowItem(row: ReaderRow) {
    when (row) {
        is ReaderRow.ChapterHeading -> Surface(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 16.dp),
            color = ZionCyan.copy(alpha = 0.1f),
            shape = RoundedCornerShape(14.dp),
        ) {
            Text(
                "CAPÍTULO ${row.number}",
                color = ZionCyan,
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 11.dp),
            )
        }
        is ReaderRow.Verse -> Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                row.number.toString().padStart(2, '0'),
                modifier = Modifier.width(28.dp),
                color = ZionGold,
                fontWeight = FontWeight.Bold,
                style = MaterialTheme.typography.labelLarge,
            )
            Text(row.text, style = MaterialTheme.typography.bodyLarge)
        }
        is ReaderRow.Marker -> Text(
            text = if (row.text.isEmpty()) "\\${row.name}" else "\\${row.name} ${row.text}",
            style = MaterialTheme.typography.bodyMedium,
            color = ZionMuted,
            fontStyle = FontStyle.Italic,
            modifier = Modifier.padding(start = 38.dp),
        )
        ReaderRow.Paragraph -> Spacer(modifier = Modifier.height(8.dp))
    }
}

@Composable
private fun AboutScreen(modifier: Modifier = Modifier) {
    val uriHandler = LocalUriHandler.current
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        item {
            ProtocolBadge("TRANSPARÊNCIA POR PADRÃO")
            Spacer(modifier = Modifier.height(14.dp))
            Text("Sobre o Zion ARC", style = MaterialTheme.typography.headlineSmall)
        }
        item {
            InfoCard("Conteúdo e licença") {
                Text(BLIVRE_ATTRIBUTION, color = ZionMuted)
                Text("Autores: Diego Santos, Mario Sérgio e Marco Teles.")
                Text("BLIVRE 2018.2.0 • CC BY 3.0 BR", color = ZionGold)
                TextButton(onClick = { uriHandler.openUri(BLIVRE_SITE) }) { Text("Site da BLIVRE") }
                TextButton(onClick = { uriHandler.openUri(BLIVRE_LICENSE) }) { Text("Ver licença") }
            }
        }
        item {
            InfoCard("Limitações do ARC v1") {
                Text(
                    "Compatível com pixels RGB8 preservados em PNG lossless. Não promete sobreviver a JPEG, resize, recorte, screenshot, impressão, câmera, correção de cor ou redes sociais.",
                    color = ZionMuted,
                )
                Text(
                    "O marcador ZARC e os identificadores públicos tornam a cápsula detectável e correlacionável. ARC não é um novo cipher, não é forensicamente invisível e não foi auditado ou formalmente verificado.",
                    color = ZionMuted,
                )
                Text(
                    "A aparência e metadados auxiliares do PNG não são autenticados. O transporte não concede permissão para descumprir leis ou regras de inspeção.",
                    color = ZionMuted,
                )
            }
        }
        item {
            InfoCard("Privacidade") {
                Text(
                    "A abertura ocorre no aparelho. O app solicita leitura de imagens, não declara permissão de internet e não persiste senha, livro ou URI selecionada.",
                    color = ZionMuted,
                )
                Text(
                    "Limitação da JVM: o campo de senha do Compose usa String imutável; essas cópias não podem ser zeradas com garantia. Os ByteArrays explícitos são limpos após a tentativa.",
                    color = ZionMuted,
                )
            }
        }
    }
}

@Composable
private fun InfoCard(title: String, content: @Composable () -> Unit) {
    Card(
        colors = CardDefaults.cardColors(containerColor = ZionPanel),
        border = BorderStroke(1.dp, ZionLine),
        shape = RoundedCornerShape(20.dp),
    ) {
        Column(
            modifier = Modifier.padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text(title, style = MaterialTheme.typography.titleLarge)
            content()
        }
    }
}

private fun userFacingDecodeError(failure: Exception): String {
    val code = failure.message.orEmpty()
    return when {
        "ZION_INVALID_IMAGE" in code -> "A imagem selecionada não é uma cápsula ARC PNG válida."
        "ZION_INVALID_PASSPHRASE" in code -> "A senha deve ter entre 1 e 1.024 bytes UTF-8."
        else -> "Senha incorreta ou a cápsula não pôde ser recuperada."
    }
}
