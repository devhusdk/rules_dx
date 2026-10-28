package dx.docs.javadoc;

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.util.DocTrees;
import com.sun.source.util.SourcePositions;
import com.sun.source.util.TreePath;
import dx.docs.jvm.ApiRecordWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import javax.lang.model.SourceVersion;
import javax.lang.model.element.AnnotationMirror;
import javax.lang.model.element.Element;
import javax.lang.model.element.ElementKind;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.Modifier;
import javax.lang.model.element.PackageElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import javax.lang.model.type.TypeKind;
import javax.lang.model.type.TypeMirror;
import javax.lang.model.util.ElementFilter;
import javax.lang.model.util.Elements;
import javax.lang.model.util.Types;
import javax.tools.Diagnostic;
import jdk.javadoc.doclet.Doclet;
import jdk.javadoc.doclet.DocletEnvironment;
import jdk.javadoc.doclet.Reporter;

/** Writes the public API of the documented sources as "jvm api v1" records. */
public final class IrDoclet implements Doclet {
  private static final List<String> TYPE_MODIFIERS =
      Arrays.asList("public", "protected", "abstract", "static", "final", "sealed", "non-sealed");
  private static final List<String> MEMBER_MODIFIERS =
      Arrays.asList(
          "public",
          "protected",
          "abstract",
          "static",
          "final",
          "default",
          "synchronized",
          "native");

  private Reporter reporter;
  private Path out;
  private Path manifest;

  private Names names;
  private Docs docs;
  private Elements elements;
  private DocTrees trees;
  private Map<Path, String> files;
  private final List<ApiRecordWriter.Symbol> symbols = new ArrayList<>();
  private final Set<Element> seen = new HashSet<>();
  private boolean failed;

  @Override
  public void init(Locale locale, Reporter reporter) {
    this.reporter = reporter;
  }

  @Override
  public String getName() {
    return "dx-ir";
  }

  @Override
  public Set<? extends Option> getSupportedOptions() {
    Set<Option> options = new LinkedHashSet<>();
    options.add(new PathOption("--dx-out", path -> out = path));
    options.add(new PathOption("--dx-manifest", path -> manifest = path));
    return options;
  }

  @Override
  public SourceVersion getSupportedSourceVersion() {
    return SourceVersion.latest();
  }

  @Override
  public boolean run(DocletEnvironment environment) {
    if (out == null || manifest == null) {
      reporter.print(Diagnostic.Kind.ERROR, "--dx-out and --dx-manifest are required");
      return false;
    }
    try {
      files = readManifest();
    } catch (IOException error) {
      reporter.print(Diagnostic.Kind.ERROR, "cannot read " + manifest + ": " + error.getMessage());
      return false;
    }
    elements = environment.getElementUtils();
    Types types = environment.getTypeUtils();
    trees = environment.getDocTrees();
    names = new Names(types);
    docs = new Docs(trees, names, elements, types);
    Set<Element> roots = new LinkedHashSet<>();
    roots.addAll(environment.getSpecifiedElements());
    roots.addAll(environment.getIncludedElements());
    for (PackageElement element : ElementFilter.packagesIn(roots)) {
      pack(element);
    }
    for (TypeElement element : ElementFilter.typesIn(roots)) {
      if (element.getEnclosingElement() instanceof PackageElement) {
        type(environment, element);
      }
    }
    if (failed) {
      return false;
    }
    try {
      ApiRecordWriter.write(out, "java", "javadoc", Runtime.version().toString(), symbols);
    } catch (IOException error) {
      reporter.print(Diagnostic.Kind.ERROR, "cannot write " + out + ": " + error.getMessage());
      return false;
    }
    return true;
  }

  private Map<Path, String> readManifest() throws IOException {
    Map<Path, String> map = new HashMap<>();
    for (String line : Files.readAllLines(manifest, StandardCharsets.UTF_8)) {
      if (line.isEmpty()) {
        continue;
      }
      String exec = line.split("\t", -1)[0];
      map.put(Paths.get(exec).toAbsolutePath().normalize(), exec);
    }
    return map;
  }

  private void pack(PackageElement element) {
    if (element.isUnnamed() || trees.getPath(element) == null || !seen.add(element)) {
      return;
    }
    emit(
        element,
        "package",
        Names.qualified(element),
        null,
        element.getQualifiedName().toString(),
        null);
  }

  private void type(DocletEnvironment environment, TypeElement element) {
    if (!seen.add(element) || !environment.isIncluded(element) || !isPublic(element)) {
      return;
    }
    if (explicit(element)) {
      emit(element, kindOf(element), Names.qualified(element), null, typeSignature(element), null);
    }
    for (Element member : element.getEnclosedElements()) {
      if (!environment.isIncluded(member) || !isPublic(member) || !explicit(member)) {
        continue;
      }
      switch (member.getKind()) {
        case CLASS:
        case INTERFACE:
        case ENUM:
        case ANNOTATION_TYPE:
          type(environment, (TypeElement) member);
          break;
        case CONSTRUCTOR:
        case METHOD:
          emit(
              member,
              member.getKind() == ElementKind.CONSTRUCTOR ? "constructor" : "method",
              Names.qualified(member),
              names.overload((ExecutableElement) member),
              executableSignature((ExecutableElement) member),
              element);
          break;
        case FIELD:
        case ENUM_CONSTANT:
          emit(
              member,
              fieldKind((VariableElement) member),
              Names.qualified(member),
              null,
              fieldSignature((VariableElement) member),
              element);
          break;
        default:
          if ("RECORD".equals(member.getKind().name())) {
            type(environment, (TypeElement) member);
          }
          break;
      }
    }
  }

  private boolean explicit(Element element) {
    return elements.getOrigin(element) == Elements.Origin.EXPLICIT;
  }

  private boolean isPublic(Element element) {
    return element.getModifiers().contains(Modifier.PUBLIC)
        || (element.getEnclosingElement() != null
            && element.getEnclosingElement().getKind() == ElementKind.INTERFACE);
  }

  private static String kindOf(TypeElement element) {
    switch (element.getKind()) {
      case INTERFACE:
        return "interface";
      case ENUM:
        return "enum";
      case ANNOTATION_TYPE:
        return "annotation";
      default:
        return "RECORD".equals(element.getKind().name()) ? "record" : "class";
    }
  }

  private static String fieldKind(VariableElement element) {
    if (element.getKind() == ElementKind.ENUM_CONSTANT) {
      return "enum_constant";
    }
    return element.getModifiers().contains(Modifier.STATIC)
            && element.getModifiers().contains(Modifier.FINAL)
            && element.getConstantValue() != null
        ? "constant"
        : "field";
  }

  private void emit(
      Element element,
      String kind,
      String qualified,
      List<String> overload,
      String signature,
      TypeElement owner) {
    TreePath path = trees.getPath(element);
    if (path == null) {
      return;
    }
    CompilationUnitTree unit = path.getCompilationUnit();
    String file = files.get(Paths.get(unit.getSourceFile().toUri()).toAbsolutePath().normalize());
    SourcePositions positions = trees.getSourcePositions();
    long position = positions.getStartPosition(unit, path.getLeaf());
    long line = position < 0 ? 0 : unit.getLineMap().getLineNumber(position);
    if (file == null || line < 1) {
      reporter.print(
          Diagnostic.Kind.ERROR,
          "cannot locate "
              + qualified
              + " in the srcs manifest ("
              + unit.getSourceFile().getName()
              + ")");
      failed = true;
      return;
    }
    Docs.Doc doc = docs.of(element);
    List<ApiRecordWriter.Param> params = new ArrayList<>();
    ApiRecordWriter.Returns returns = null;
    if (element instanceof ExecutableElement) {
      ExecutableElement executable = (ExecutableElement) element;
      for (VariableElement parameter : executable.getParameters()) {
        String name = parameter.getSimpleName().toString();
        params.add(
            new ApiRecordWriter.Param(
                name, names.type(parameter.asType()), doc.params.getOrDefault(name, "")));
      }
      if (executable.getReturnType().getKind() != TypeKind.VOID
          && element.getKind() == ElementKind.METHOD) {
        returns =
            new ApiRecordWriter.Returns(
                names.type(executable.getReturnType()), doc.returns == null ? "" : doc.returns);
      }
    }
    String memberOf;
    if (owner != null) {
      memberOf = Names.qualified(owner);
    } else if (element instanceof TypeElement) {
      Element parent = element.getEnclosingElement();
      memberOf =
          parent instanceof PackageElement && !((PackageElement) parent).isUnnamed()
              ? ((PackageElement) parent).getQualifiedName().toString()
              : parent instanceof TypeElement ? Names.qualified(parent) : null;
    } else {
      memberOf = null;
    }
    List<String> implementsTypes = new ArrayList<>();
    if (element instanceof TypeElement) {
      TypeElement type = (TypeElement) element;
      TypeMirror superclass = type.getSuperclass();
      if (superclass.getKind() == TypeKind.DECLARED) {
        String name = names.erasedQualified(superclass);
        if (!"java.lang.Object".equals(name)
            && !"java.lang.Enum".equals(name)
            && !"java.lang.Record".equals(name)) {
          implementsTypes.add(name);
        }
      }
      for (TypeMirror face : type.getInterfaces()) {
        implementsTypes.add(names.erasedQualified(face));
      }
    }
    symbols.add(
        new ApiRecordWriter.Symbol(
            qualified,
            overload,
            kind,
            signature,
            doc.markdown(),
            params,
            returns,
            file,
            (int) line,
            memberOf,
            implementsTypes));
  }

  private String annotations(Element element) {
    StringBuilder out = new StringBuilder();
    for (AnnotationMirror mirror : element.getAnnotationMirrors()) {
      Element annotation = mirror.getAnnotationType().asElement();
      boolean documented = false;
      for (AnnotationMirror meta : annotation.getAnnotationMirrors()) {
        if (((TypeElement) meta.getAnnotationType().asElement())
            .getQualifiedName()
            .contentEquals("java.lang.annotation.Documented")) {
          documented = true;
        }
      }
      if (documented) {
        String text = mirror.toString();
        int open = text.indexOf('(');
        String head = open < 0 ? text : text.substring(0, open);
        String simple = "@" + head.substring(head.lastIndexOf('.') + 1);
        out.append(simple).append(open < 0 ? "" : text.substring(open)).append(' ');
      }
    }
    return out.toString();
  }

  private String modifiers(Element element, List<String> allowed, boolean inInterface) {
    StringBuilder out = new StringBuilder();
    for (String name : allowed) {
      for (Modifier modifier : element.getModifiers()) {
        if (name.equals(modifier.toString())) {
          if (inInterface && ("public".equals(name) || "abstract".equals(name))) {
            continue;
          }
          out.append(name).append(' ');
        }
      }
    }
    return out.toString();
  }

  private String typeSignature(TypeElement element) {
    ElementKind kind = element.getKind();
    boolean face = kind == ElementKind.INTERFACE || kind == ElementKind.ANNOTATION_TYPE;
    List<String> allowed = new ArrayList<>(TYPE_MODIFIERS);
    if (face) {
      allowed.remove("abstract");
      allowed.remove("static");
    } else if (kind == ElementKind.ENUM || "RECORD".equals(kind.name())) {
      allowed.remove("final");
      allowed.remove("abstract");
      allowed.remove("static");
    }
    StringBuilder out = new StringBuilder(annotations(element));
    out.append(modifiers(element, allowed, false));
    String keyword;
    switch (kind) {
      case INTERFACE:
        keyword = "interface";
        break;
      case ENUM:
        keyword = "enum";
        break;
      case ANNOTATION_TYPE:
        keyword = "@interface";
        break;
      default:
        keyword = "RECORD".equals(kind.name()) ? "record" : "class";
        break;
    }
    out.append(keyword).append(' ').append(element.getSimpleName());
    out.append(names.typeParameters(element.getTypeParameters()));
    TypeMirror superclass = element.getSuperclass();
    if (kind == ElementKind.CLASS
        && superclass.getKind() == TypeKind.DECLARED
        && !"Object".equals(names.type(superclass))) {
      out.append(" extends ").append(names.type(superclass));
    }
    List<String> faces = new ArrayList<>();
    for (TypeMirror mirror : element.getInterfaces()) {
      faces.add(names.type(mirror));
    }
    if (!faces.isEmpty() && kind != ElementKind.ANNOTATION_TYPE) {
      out.append(face ? " extends " : " implements ").append(String.join(", ", faces));
    }
    List<String> permitted = new ArrayList<>();
    for (TypeMirror mirror : permitted(element)) {
      permitted.add(names.type(mirror));
    }
    if (!permitted.isEmpty()) {
      out.append(" permits ").append(String.join(", ", permitted));
    }
    return out.toString();
  }

  @SuppressWarnings("unchecked")
  private static List<? extends TypeMirror> permitted(TypeElement element) {
    try {
      return (List<? extends TypeMirror>)
          TypeElement.class.getMethod("getPermittedSubclasses").invoke(element);
    } catch (ReflectiveOperationException | RuntimeException error) {
      return new ArrayList<TypeMirror>();
    }
  }

  private String executableSignature(ExecutableElement element) {
    boolean inInterface = element.getEnclosingElement().getKind() == ElementKind.INTERFACE;
    StringBuilder out = new StringBuilder(annotations(element));
    out.append(modifiers(element, MEMBER_MODIFIERS, inInterface));
    String generics = names.typeParameters(element.getTypeParameters());
    if (!generics.isEmpty()) {
      out.append(generics).append(' ');
    }
    if (element.getKind() == ElementKind.METHOD) {
      out.append(names.type(element.getReturnType())).append(' ');
      out.append(element.getSimpleName());
    } else {
      out.append(element.getEnclosingElement().getSimpleName());
    }
    out.append('(');
    List<? extends VariableElement> parameters = element.getParameters();
    for (int index = 0; index < parameters.size(); index++) {
      if (index > 0) {
        out.append(", ");
      }
      TypeMirror type = parameters.get(index).asType();
      String printed = names.type(type);
      if (index == parameters.size() - 1 && element.isVarArgs() && printed.endsWith("[]")) {
        printed = printed.substring(0, printed.length() - 2) + "...";
      }
      out.append(printed).append(' ').append(parameters.get(index).getSimpleName());
    }
    out.append(')');
    List<String> thrown = new ArrayList<>();
    for (TypeMirror mirror : element.getThrownTypes()) {
      thrown.add(names.type(mirror));
    }
    if (!thrown.isEmpty()) {
      out.append(" throws ").append(String.join(", ", thrown));
    }
    return out.toString();
  }

  private String fieldSignature(VariableElement element) {
    if (element.getKind() == ElementKind.ENUM_CONSTANT) {
      return annotations(element) + element.getSimpleName();
    }
    boolean inInterface = element.getEnclosingElement().getKind() == ElementKind.INTERFACE;
    StringBuilder out = new StringBuilder(annotations(element));
    out.append(
        modifiers(
            element,
            Arrays.asList("public", "protected", "static", "final", "transient", "volatile"),
            inInterface));
    out.append(names.type(element.asType())).append(' ').append(element.getSimpleName());
    Object value = element.getConstantValue();
    if (value != null && "constant".equals(fieldKind(element))) {
      out.append(" = ").append(literal(value));
    }
    return out.toString();
  }

  private static String literal(Object value) {
    if (value instanceof String) {
      StringBuilder out = new StringBuilder("\"");
      for (char ch : ((String) value).toCharArray()) {
        if (ch == '"' || ch == '\\') {
          out.append('\\').append(ch);
        } else if (ch == '\n') {
          out.append("\\n");
        } else if (ch == '\r') {
          out.append("\\r");
        } else if (ch == '\t') {
          out.append("\\t");
        } else if (ch < 0x20) {
          out.append(String.format(Locale.ROOT, "\\u%04x", (int) ch));
        } else {
          out.append(ch);
        }
      }
      return out.append('"').toString();
    }
    if (value instanceof Character) {
      char ch = (Character) value;
      return ch == '\'' || ch == '\\'
          ? "'\\" + ch + "'"
          : ch < 0x20 ? String.format(Locale.ROOT, "'\\u%04x'", (int) ch) : "'" + ch + "'";
    }
    if (value instanceof Long) {
      return value + "L";
    }
    if (value instanceof Float) {
      return value + "f";
    }
    return value.toString();
  }

  /** A doclet option that takes one path. */
  private static final class PathOption implements Option {
    private final String name;
    private final java.util.function.Consumer<Path> sink;

    PathOption(String name, java.util.function.Consumer<Path> sink) {
      this.name = name;
      this.sink = sink;
    }

    @Override
    public int getArgumentCount() {
      return 1;
    }

    @Override
    public String getDescription() {
      return "a path written or read by the dx documentation adapter";
    }

    @Override
    public Kind getKind() {
      return Kind.OTHER;
    }

    @Override
    public List<String> getNames() {
      return List.of(name);
    }

    @Override
    public String getParameters() {
      return "<path>";
    }

    @Override
    public boolean process(String option, List<String> arguments) {
      sink.accept(Paths.get(arguments.get(0)));
      return true;
    }
  }
}
