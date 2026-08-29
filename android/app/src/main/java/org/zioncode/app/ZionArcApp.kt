package org.zioncode.app

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.KeyboardType
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

    val openDocument = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        selectedUri = uri
        errorMessage = null
        notice = if (uri == null) null else "Arte PNG selecionada. Digite a senha para abrir."
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

    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
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
                        selected = selectedUri != null,
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
                                errorMessage =
                                    "A senha deve ter entre 1 e 1.024 bytes UTF-8."
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

@Composable
private fun AppBar(screen: Screen, onHome: () -> Unit, onAbout: () -> Unit) {
    Surface(tonalElevation = 3.dp) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Zion ARC",
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.weight(1f),
            )
            if (screen == Screen.About) {
                TextButton(onClick = onHome) { Text("Voltar") }
            } else {
                TextButton(onClick = onAbout) { Text("Sobre") }
            }
        }
    }
}

@Composable
private fun ImportScreen(
    selected: Boolean,
    password: String,
    loading: Boolean,
    errorMessage: String?,
    notice: String?,
    onPasswordChanged: (String) -> Unit,
    onSelect: () -> Unit,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Abra um livro que viaja como arte", style = MaterialTheme.typography.headlineSmall)
        Text(
            "Selecione o PNG original. Capturas de tela, JPEG, redimensionamento e filtros não são compatíveis.",
        )
        Button(onClick = onSelect, enabled = !loading) {
            Text(if (selected) "Trocar arte PNG" else "Selecionar arte PNG")
        }
        if (selected) {
            OutlinedTextField(
                value = password,
                onValueChange = onPasswordChanged,
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Senha") },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(
                    autoCorrectEnabled = false,
                    keyboardType = KeyboardType.Password,
                ),
                enabled = !loading,
                supportingText = { Text("Máximo de 1.024 bytes UTF-8.") },
            )
            Button(
                onClick = onOpen,
                enabled = password.isNotEmpty() && !loading,
            ) {
                Text("Abrir offline")
            }
        }
        if (loading) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CircularProgressIndicator()
                Text("Autenticando e recuperando o livro…")
            }
        }
        notice?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
        errorMessage?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Spacer(modifier = Modifier.weight(1f))
        Text(
            "O leitor não usa internet, conta, anúncios, analytics nem modelo de IA.",
            style = MaterialTheme.typography.bodySmall,
        )
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
    Column(modifier = modifier.fillMaxSize()) {
        Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp)) {
            Text("${book.ordinal}. ${book.title}", style = MaterialTheme.typography.headlineSmall)
            Text(
                "${book.sourceName} ${book.sourceVersion}",
                style = MaterialTheme.typography.bodySmall,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = onExport) { Text("Exportar USFM") }
                TextButton(onClick = onLock) { Text("Fechar livro") }
            }
            notice?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
        }
        LazyColumn(
            contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(book.rows) { row -> ReaderRowItem(row) }
        }
    }
}

@Composable
private fun ReaderRowItem(row: ReaderRow) {
    when (row) {
        is ReaderRow.ChapterHeading -> Text(
            "Capítulo ${row.number}",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(top = 12.dp),
        )
        is ReaderRow.Verse -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(row.number.toString(), fontWeight = FontWeight.Bold)
            Text(row.text, style = MaterialTheme.typography.bodyLarge)
        }
        is ReaderRow.Marker -> Text(
            text = if (row.text.isEmpty()) "\\${row.name}" else "\\${row.name} ${row.text}",
            style = MaterialTheme.typography.bodyMedium,
            fontStyle = FontStyle.Italic,
        )
        ReaderRow.Paragraph -> Spacer(modifier = Modifier.height(8.dp))
    }
}

@Composable
private fun AboutScreen(modifier: Modifier = Modifier) {
    val uriHandler = LocalUriHandler.current
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item { Text("Sobre e licenças", style = MaterialTheme.typography.headlineSmall) }
        item { Text(BLIVRE_ATTRIBUTION) }
        item {
            Text("Autores: Diego Santos, Mario Sérgio e Marco Teles.")
            Text("Versão da fonte: BLIVRE 2018.2.0 (fevereiro de 2018).")
            Text("Licença do conteúdo: Creative Commons Atribuição 3.0 Brasil — CC BY 3.0 BR.")
        }
        item {
            TextButton(onClick = { uriHandler.openUri(BLIVRE_SITE) }) {
                Text(BLIVRE_SITE)
            }
            TextButton(onClick = { uriHandler.openUri(BLIVRE_LICENSE) }) {
                Text(BLIVRE_LICENSE)
            }
        }
        item {
            Text("Limitações do ARC v1", style = MaterialTheme.typography.titleMedium)
            Text(
                "Compatível com pixels RGB8 preservados em PNG lossless. Não promete sobreviver a JPEG, resize, recorte, screenshot, impressão, câmera, correção de cor ou redes sociais.",
            )
            Text(
                "O marcador ZARC e os identificadores públicos tornam a cápsula detectável e correlacionável. ARC não é um novo cipher, não é forensicamente invisível e não foi auditado ou formalmente verificado.",
            )
            Text(
                "A aparência e metadados auxiliares do PNG não são autenticados. O transporte não concede permissão para descumprir leis ou regras de inspeção.",
            )
        }
        item {
            Text("Privacidade", style = MaterialTheme.typography.titleMedium)
            Text(
                "A abertura ocorre no aparelho. O app não declara permissão de internet e não persiste senha, livro ou URI selecionada.",
            )
            Text(
                "Limitação da JVM: o campo de senha do Compose usa String imutável; essas cópias não podem ser zeradas com garantia. Os ByteArrays explícitos são limpos após a tentativa.",
            )
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
