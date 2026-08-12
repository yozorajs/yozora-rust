use crate::weaver::*;
use crate::{MarkupWeaver, MarkupWeaverContract, NodeWeaver};

pub struct DefaultMarkupWeaver {
    inner: MarkupWeaver,
}

impl Default for DefaultMarkupWeaver {
    fn default() -> Self {
        let mut inner = MarkupWeaver::new();
        inner
            .use_weaver(Box::new(RootWeaver), false)
            .use_weaver(Box::new(AdmonitionWeaver), false)
            .use_weaver(Box::new(BlockquoteWeaver), false)
            .use_weaver(Box::new(BreakWeaver), false)
            .use_weaver(Box::new(CodeWeaver), false)
            .use_weaver(Box::new(DefinitionWeaver), false)
            .use_weaver(Box::new(DeleteWeaver), false)
            .use_weaver(Box::new(EcmaImportWeaver), false)
            .use_weaver(Box::new(EmphasisWeaver), false)
            .use_weaver(Box::new(FootnoteWeaver), false)
            .use_weaver(Box::new(FootnoteDefinitionWeaver), false)
            .use_weaver(Box::new(FootnoteReferenceWeaver), false)
            .use_weaver(Box::new(FrontmatterWeaver), false)
            .use_weaver(Box::new(HeadingWeaver), false)
            .use_weaver(Box::new(HtmlWeaver), false)
            .use_weaver(Box::new(ImageWeaver), false)
            .use_weaver(Box::new(ImageReferenceWeaver), false)
            .use_weaver(Box::new(InlineCodeWeaver), false)
            .use_weaver(Box::new(InlineMathWeaver::default()), false)
            .use_weaver(Box::new(LinkWeaver), false)
            .use_weaver(Box::new(LinkReferenceWeaver), false)
            .use_weaver(Box::new(ListWeaver), false)
            .use_weaver(Box::new(ListItemWeaver), false)
            .use_weaver(Box::new(MathWeaver), false)
            .use_weaver(Box::new(ParagraphWeaver), false)
            .use_weaver(Box::new(StrongWeaver), false)
            .use_weaver(Box::new(TableWeaver), false)
            .use_weaver(Box::new(TextWeaver), false)
            .use_weaver(Box::new(ThematicBreakWeaver), false);
        Self { inner }
    }
}

impl DefaultMarkupWeaver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MarkupWeaverContract for DefaultMarkupWeaver {
    fn use_weaver(&mut self, weaver: Box<dyn NodeWeaver>, force_replace: bool) -> &mut Self {
        self.inner.use_weaver(weaver, force_replace);
        self
    }

    fn unmount_weaver(&mut self, node_type: &str) -> &mut Self {
        self.inner.unmount_weaver(node_type);
        self
    }

    fn weave(&self, ast: &yozora_ast::Root) -> String {
        self.inner.weave(ast)
    }
}
